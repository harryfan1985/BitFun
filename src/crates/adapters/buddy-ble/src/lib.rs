//! BLE transport adapter for the Buddy hardware approval bridge.
//!
//! Pure transport layer: scan for `Claude-*` devices, connect, and move
//! newline-delimited JSON lines over the Nordic UART Service (NUS). It owns
//! no protocol semantics — the caller decides what each JSON line means.

use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::Stream;
use std::pin::Pin;
use std::time::Duration;
use uuid::Uuid;

/// Nordic UART Service (NUS) UUIDs.
const NUS_RX_UUID: &str = "6e400002-b5a3-f393-e0a9-e50e24dcca9e"; // write (host -> device)
const NUS_TX_UUID: &str = "6e400003-b5a3-f393-e0a9-e50e24dcca9e"; // notify (device -> host)

/// BLE transport errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no BLE adapter available")]
    NoAdapter,
    #[error("no Claude-* device found")]
    DeviceNotFound,
    #[error("NUS characteristic not found: {0}")]
    CharacteristicNotFound(&'static str),
    #[error("BLE operation failed: {0}")]
    Ble(#[from] btleplug::Error),
    #[error("not connected")]
    NotConnected,
}

/// Identifies a connected device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
}

/// Notification stream yielded by the platform.
type NotificationStream = Pin<Box<dyn Stream<Item = btleplug::api::ValueNotification> + Send>>;

/// BLE transport that connects to the M5StickC Buddy over NUS.
pub struct BuddyBleTransport {
    adapter: Adapter,
    peripheral: Option<Peripheral>,
    rx: Option<Characteristic>,
    /// Wrapped in a mutex because the platform notification stream is `Send`
    /// but not `Sync`; the mutex makes the transport itself `Sync`.
    notifications: Option<tokio::sync::Mutex<NotificationStream>>,
    /// Partial line accumulated across MTU-fragmented notifications.
    line_buf: Vec<u8>,
}

impl BuddyBleTransport {
    /// Create a transport bound to the first available BLE adapter.
    pub async fn new() -> Result<Self, Error> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NoAdapter)?;
        Ok(Self {
            adapter,
            peripheral: None,
            rx: None,
            notifications: None,
            line_buf: Vec::new(),
        })
    }

    /// Scan for `Claude-*` devices and connect to the first one found.
    pub async fn scan_and_connect(&mut self, scan_timeout: Duration) -> Result<DeviceInfo, Error> {
        self.adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(scan_timeout).await;
        self.adapter.stop_scan().await?;

        let mut target = None;
        for peripheral in self.adapter.peripherals().await? {
            if let Ok(Some(props)) = peripheral.properties().await {
                let name = props.local_name.unwrap_or_default();
                if name.starts_with("Claude-") {
                    target = Some((peripheral, name));
                    break;
                }
            }
        }

        let (peripheral, name) = target.ok_or(Error::DeviceNotFound)?;
        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let rx_uuid = Uuid::parse_str(NUS_RX_UUID).expect("valid NUS RX UUID");
        let tx_uuid = Uuid::parse_str(NUS_TX_UUID).expect("valid NUS TX UUID");
        let chars = peripheral.characteristics();
        let rx = chars
            .iter()
            .find(|c| c.uuid == rx_uuid)
            .cloned()
            .ok_or(Error::CharacteristicNotFound("NUS RX"))?;
        let tx = chars
            .iter()
            .find(|c| c.uuid == tx_uuid)
            .cloned()
            .ok_or(Error::CharacteristicNotFound("NUS TX"))?;

        // Subscribe before acquiring the notification stream. On a first-time
        // device, macOS triggers pairing here (accessing the encrypted CCCD).
        peripheral.subscribe(&tx).await?;
        self.notifications = Some(tokio::sync::Mutex::new(peripheral.notifications().await?));
        self.rx = Some(rx);
        self.peripheral = Some(peripheral);

        log::info!("Buddy BLE connected to {}", name);
        Ok(DeviceInfo { name })
    }

    /// Write one newline-delimited JSON line to the device.
    pub async fn write_line(&self, json: &str) -> Result<(), Error> {
        let peripheral = self.peripheral.as_ref().ok_or(Error::NotConnected)?;
        let rx = self.rx.as_ref().ok_or(Error::NotConnected)?;

        let mut data = json.as_bytes().to_vec();
        data.push(b'\n');
        peripheral
            .write(rx, &data, WriteType::WithoutResponse)
            .await?;
        Ok(())
    }

    /// Read the next complete newline-delimited JSON line, reassembling across
    /// MTU-fragmented notifications. Returns `Ok(None)` on timeout.
    pub async fn next_line(&mut self, timeout: Duration) -> Result<Option<String>, Error> {
        let notifications = self.notifications.as_ref().ok_or(Error::NotConnected)?;
        let mut stream = notifications.lock().await;

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }

            use futures::StreamExt;
            match tokio::time::timeout(remaining, stream.next()).await {
                Ok(Some(notification)) => {
                    self.line_buf.extend_from_slice(&notification.value);
                    if let Some(pos) = self.line_buf.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = self.line_buf.drain(..=pos).collect();
                        let text = String::from_utf8_lossy(&line).trim().to_string();
                        if text.is_empty() {
                            continue;
                        }
                        return Ok(Some(text));
                    }
                }
                Ok(None) => return Ok(None),
                Err(_) => return Ok(None),
            }
        }
    }

    /// Whether a device is currently connected.
    pub fn is_connected(&self) -> bool {
        self.peripheral.is_some()
    }

    /// Disconnect from the device.
    pub async fn disconnect(&mut self) {
        if let Some(peripheral) = self.peripheral.take() {
            let _ = peripheral.disconnect().await;
        }
        self.rx = None;
        self.notifications = None;
        self.line_buf.clear();
        log::info!("Buddy BLE disconnected");
    }
}
