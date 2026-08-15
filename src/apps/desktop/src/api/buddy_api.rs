//! Tauri commands for the Buddy hardware approval bridge.
//!
//! Provides configuration and status for the Buddy feature. Buddy is a local
//! hardware peripheral and is currently macOS-only (CoreBluetooth BLE).

use crate::api::app_state::AppState;
use bitfun_core::agentic::buddy::get_global_buddy_runtime;
use bitfun_core::service::config::global::GlobalConfigManager;
use bitfun_core::service::config::types::BuddyConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Tauri command types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuddyStatusResponse {
    pub enabled: bool,
    pub state: String,
    pub bridge_online: bool,
    pub device_name: Option<String>,
    pub device_connected: bool,
    pub pending_prompts: usize,
}

// ── Tauri commands ──────────────────────────────────────────────────

#[tauri::command]
pub async fn buddy_get_config(_state: State<'_, AppState>) -> Result<BuddyConfig, String> {
    let service = GlobalConfigManager::get_service()
        .await
        .map_err(|e| e.to_string())?;
    service
        .get_config::<BuddyConfig>(Some("buddy"))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn buddy_get_status(_state: State<'_, AppState>) -> Result<BuddyStatusResponse, String> {
    let service = GlobalConfigManager::get_service()
        .await
        .map_err(|e| e.to_string())?;
    let config: BuddyConfig = service
        .get_config(Some("buddy"))
        .await
        .map_err(|e| e.to_string())?;

    let (bridge_online, device_name, device_connected, pending_prompts, state) =
        if let Some(runtime) = get_global_buddy_runtime() {
            let connected = runtime.is_connected().await;
            (
                connected,
                runtime.device_name(),
                connected,
                runtime.state().pending_count(),
                if connected {
                    "connected"
                } else {
                    "scanning_for_device"
                }
                .to_string(),
            )
        } else {
            (false, None, false, 0, "not_configured".to_string())
        };

    Ok(BuddyStatusResponse {
        enabled: config.enabled,
        state,
        bridge_online,
        device_name,
        device_connected,
        pending_prompts,
    })
}

#[tauri::command]
pub async fn buddy_set_config(
    _state: State<'_, AppState>,
    request: BuddyConfig,
) -> Result<(), String> {
    let service = GlobalConfigManager::get_service()
        .await
        .map_err(|e| e.to_string())?;
    service
        .set_config("buddy", request.clone())
        .await
        .map_err(|e| e.to_string())?;

    log::info!(
        "Buddy config updated: enabled={}. Restart may be required for changes to take effect.",
        request.enabled
    );
    Ok(())
}

#[tauri::command]
pub async fn buddy_test_connection() -> Result<bool, String> {
    let runtime = get_global_buddy_runtime().ok_or_else(|| "Buddy not started".to_string())?;
    Ok(runtime.is_connected().await)
}

#[tauri::command]
pub async fn buddy_check_prerequisites() -> Result<BuddyPrerequisites, String> {
    let os = std::env::consts::OS;
    // Buddy is currently macOS-only (CoreBluetooth BLE backend).
    Ok(BuddyPrerequisites {
        supported: os == "macos",
        os: os.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuddyPrerequisites {
    /// Whether the current host OS is supported by Buddy (currently macOS only).
    pub supported: bool,
    /// Host OS identifier: "macos", "windows", or "linux".
    pub os: String,
}
