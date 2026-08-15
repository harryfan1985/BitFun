//! Pending approval tracking for the Buddy hardware bridge.
//!
//! Owns the small in-memory set of tool ids currently waiting for a physical
//! button decision, plus their timeout deadlines.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Thread-safe pending-approval registry.
#[derive(Debug, Default)]
pub struct ApprovalState {
    /// tool_id -> timeout deadline (None = no timeout).
    inner: Mutex<HashMap<String, Option<Instant>>>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool id as pending with an optional timeout.
    pub fn insert(&self, tool_id: &str, timeout: Option<Duration>) {
        let timeout_at = timeout.map(|d| Instant::now() + d);
        self.inner
            .lock()
            .unwrap()
            .insert(tool_id.to_string(), timeout_at);
    }

    /// Remove a pending tool id. Returns true if it was present.
    pub fn remove(&self, tool_id: &str) -> bool {
        self.inner.lock().unwrap().remove(tool_id).is_some()
    }

    /// Number of pending prompts.
    pub fn pending_count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Collect tool ids whose timeout has expired, removing them.
    pub fn drain_expired(&self) -> Vec<String> {
        let now = Instant::now();
        let mut guard = self.inner.lock().unwrap();
        let expired: Vec<String> = guard
            .iter()
            .filter(|(_, t)| t.is_some_and(|at| at <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            guard.remove(id);
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_remove_round_trip() {
        let state = ApprovalState::new();
        state.insert("tool_1", None);
        assert_eq!(state.pending_count(), 1);
        assert!(state.remove("tool_1"));
        assert_eq!(state.pending_count(), 0);
    }

    #[test]
    fn expired_prompts_drain() {
        let state = ApprovalState::new();
        state.insert("tool_1", Some(Duration::from_millis(1)));
        state.insert("tool_2", None);
        std::thread::sleep(Duration::from_millis(10));
        let expired = state.drain_expired();
        assert_eq!(expired, vec!["tool_1"]);
        assert_eq!(state.pending_count(), 1);
    }
}
