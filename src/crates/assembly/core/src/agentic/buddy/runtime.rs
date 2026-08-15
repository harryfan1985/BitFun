//! Buddy runtime: owns the BLE transport and pending approvals, and drives the
//! device notification loop.

use super::approval_state::ApprovalState;
use super::protocol::{self, Heartbeat, PermissionChoice};
use crate::agentic::coordination::get_global_coordinator;
use bitfun_agent_runtime::sdk::PermissionReply;
use bitfun_buddy_ble::BuddyBleTransport;
use log::{debug, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;

/// How long the notification loop waits for a device message each iteration.
const NOTIFY_POLL: Duration = Duration::from_millis(200);

static GLOBAL_BUDDY_RUNTIME: OnceLock<Arc<BuddyRuntime>> = OnceLock::new();

/// Install the process-wide Buddy runtime (called once at boot).
pub fn install_buddy_runtime(runtime: Arc<BuddyRuntime>) {
    let _ = GLOBAL_BUDDY_RUNTIME.set(runtime);
}

/// Get the process-wide Buddy runtime, if installed.
pub fn get_global_buddy_runtime() -> Option<Arc<BuddyRuntime>> {
    GLOBAL_BUDDY_RUNTIME.get().cloned()
}

/// Shared Buddy runtime.
pub struct BuddyRuntime {
    transport: Mutex<BuddyBleTransport>,
    state: Arc<ApprovalState>,
    device_name: RwLock<Option<String>>,
    running: AtomicBool,
}

impl BuddyRuntime {
    pub async fn new() -> Result<Self, bitfun_buddy_ble::Error> {
        Ok(Self {
            transport: Mutex::new(BuddyBleTransport::new().await?),
            state: Arc::new(ApprovalState::new()),
            device_name: RwLock::new(None),
            running: AtomicBool::new(false),
        })
    }

    /// Shared approval state handle.
    pub fn state(&self) -> Arc<ApprovalState> {
        Arc::clone(&self.state)
    }

    /// Currently connected device name, if any.
    pub fn device_name(&self) -> Option<String> {
        self.device_name.read().unwrap().clone()
    }

    /// Scan and connect to the device, then start the notification loop.
    pub async fn start(&self) -> Result<String, String> {
        let mut transport = self.transport.lock().await;
        let info = transport
            .scan_and_connect(Duration::from_secs(5))
            .await
            .map_err(|e| e.to_string())?;

        *self.device_name.write().unwrap() = Some(info.name.clone());

        // Sync time so the device RTC is valid.
        let now = chrono::Utc::now().timestamp();
        let offset = chrono::Local::now().offset().local_minus_utc();
        let line = protocol::encode_time_sync(now, offset);
        let _ = transport.write_line(&line).await;

        self.running.store(true, Ordering::SeqCst);
        drop(transport);

        let runtime =
            get_global_buddy_runtime().ok_or_else(|| "buddy runtime not installed".to_string())?;
        tokio::spawn(async move {
            runtime.notification_loop().await;
        });

        Ok(info.name)
    }

    /// Push a heartbeat and record the prompt as pending.
    pub async fn push_heartbeat(
        &self,
        heartbeat: Heartbeat,
        timeout: Option<Duration>,
    ) -> Result<(), String> {
        let transport = self.transport.lock().await;
        let line = protocol::encode_heartbeat(&heartbeat);
        transport
            .write_line(&line)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(tool_id) = heartbeat.prompt_id.as_deref() {
            self.state.insert(tool_id, timeout);
        }
        Ok(())
    }

    /// Push a clear command and drop the pending entry.
    pub async fn push_clear(&self, tool_id: &str) -> Result<(), String> {
        let transport = self.transport.lock().await;
        let line = protocol::encode_clear(tool_id);
        transport
            .write_line(&line)
            .await
            .map_err(|e| e.to_string())?;
        self.state.remove(tool_id);
        Ok(())
    }

    /// Whether the BLE transport is connected.
    pub async fn is_connected(&self) -> bool {
        self.transport.lock().await.is_connected()
    }

    /// Stop the runtime.
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.transport.lock().await.disconnect().await;
    }

    /// Read device notifications and resolve permission decisions.
    async fn notification_loop(self: Arc<Self>) {
        debug!("Buddy notification loop started");
        while self.running.load(Ordering::SeqCst) {
            // Auto-deny prompts whose timeout has expired, mirroring the
            // previous bridge's timeout monitor.
            for tool_id in self.state.drain_expired() {
                debug!("Buddy: auto-denying expired prompt {}", tool_id);
                if let Some(coordinator) = get_global_coordinator() {
                    let _ = coordinator
                        .reply_to_tool(&tool_id, PermissionReply::Reject { feedback: None })
                        .await;
                }
                let _ = self.push_clear(&tool_id).await;
            }

            let line = {
                let mut transport = self.transport.lock().await;
                match transport.next_line(NOTIFY_POLL).await {
                    Ok(Some(line)) => line,
                    Ok(None) => continue,
                    Err(e) => {
                        debug!("Buddy notification read error: {e}");
                        continue;
                    }
                }
            };

            let Some(decision) = protocol::decode_permission(&line) else {
                continue;
            };

            let Some(coordinator) = get_global_coordinator() else {
                warn!("Buddy: coordinator unavailable for permission reply");
                continue;
            };
            let reply = match decision.decision {
                PermissionChoice::Once => PermissionReply::Once,
                PermissionChoice::Deny => PermissionReply::Reject { feedback: None },
            };
            if let Err(e) = coordinator.reply_to_tool(&decision.id, reply).await {
                warn!("Buddy: reply_to_tool failed for {}: {}", decision.id, e);
                continue;
            }

            self.state.remove(&decision.id);
            let _ = self.push_clear(&decision.id).await;
            debug!("Buddy: resolved permission for {}", decision.id);
        }
        debug!("Buddy notification loop stopped");
    }
}
