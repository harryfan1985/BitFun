//! Buddy hardware approval bridge.
//!
//! Routes tool confirmation requests to an M5StickC Plus physical device over
//! BLE (Nordic UART Service) and resolves the physical button decision back
//! through the coordinator. The BLE transport lives in the `buddy-ble`
//! adapter crate; this module owns the protocol, pending-approval state, and
//! the runtime that ties them together.

pub mod approval_state;
pub mod protocol;
pub mod runtime;
pub mod subscriber;

#[cfg(test)]
mod sanity {
    #[test]
    fn buddy_module_compiled() {
        assert!(true);
    }
}

pub use approval_state::ApprovalState;
pub use protocol::{Heartbeat, PermissionChoice, PermissionDecision};
pub use runtime::{get_global_buddy_runtime, install_buddy_runtime, BuddyRuntime};
pub use subscriber::BuddyEventSubscriber;
