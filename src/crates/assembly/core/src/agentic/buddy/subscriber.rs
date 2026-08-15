//! Event subscriber that pushes tool confirmation state to the Buddy device.

use super::protocol::Heartbeat;
use super::runtime::get_global_buddy_runtime;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use crate::service::token_usage::{get_global_token_usage_service, TimeRange, TokenUsageQuery};
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use bitfun_events::ToolEventData;
use log::{debug, warn};
use std::time::Duration;

/// Event subscriber that forwards tool confirmation events to the Buddy device.
pub struct BuddyEventSubscriber;

impl BuddyEventSubscriber {
    pub fn new() -> Self {
        Self
    }

    async fn push_state(
        &self,
        tool_id: &str,
        tool_name: &str,
        hint: &str,
        timeout: Option<Duration>,
    ) {
        // Aggregate in-memory session counts when the coordinator is available;
        // fall back to approval-only signals otherwise.
        let (total, running, waiting) = if let Some(coordinator) = get_global_coordinator() {
            let stats = coordinator.buddy_heartbeat_stats();
            (stats.total, stats.running, stats.waiting.max(1))
        } else {
            (0, 0, 1)
        };

        // Token counters are best-effort.
        let (tokens, tokens_today) = match get_global_token_usage_service() {
            Some(service) => {
                let all = service
                    .get_summary(token_query(TimeRange::All))
                    .await
                    .map(|s| s.total_output)
                    .unwrap_or(0);
                let today = service
                    .get_summary(token_query(TimeRange::Today))
                    .await
                    .map(|s| s.total_output)
                    .unwrap_or(0);
                (all, today)
            }
            None => (0, 0),
        };

        let heartbeat = Heartbeat {
            total,
            running,
            waiting,
            msg: format!("approve: {}", tool_name),
            tokens,
            tokens_today,
            prompt_id: Some(tool_id.to_string()),
            prompt_tool: tool_name.to_string(),
            prompt_hint: hint.to_string(),
        };

        let Some(runtime) = get_global_buddy_runtime() else {
            debug!("Buddy runtime not installed; dropping state push");
            return;
        };
        if let Err(e) = runtime.push_heartbeat(heartbeat, timeout).await {
            warn!("Buddy state push failed: {}", e);
        } else {
            debug!(
                "Buddy state pushed: tool_id={}",
                truncate_for_log(tool_id, 20)
            );
        }
    }

    async fn push_clear(&self, tool_id: &str) {
        let Some(runtime) = get_global_buddy_runtime() else {
            return;
        };
        if let Err(e) = runtime.push_clear(tool_id).await {
            warn!("Buddy clear failed: {}", e);
        }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for BuddyEventSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        let AgenticEvent::ToolEvent { tool_event, .. } = event else {
            return Ok(());
        };

        let identity = tool_event.identity();
        let tool_id = identity.tool_id.as_str();
        let tool_name = identity.effective_name();

        match tool_event {
            ToolEventData::ConfirmationNeeded {
                params, timeout_at, ..
            } => {
                let hint = make_hint(params);
                let timeout = timeout_at.map(|at| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let remaining = at.saturating_sub(now);
                    Duration::from_secs(remaining)
                });
                self.push_state(tool_id, tool_name, &hint, timeout).await;
            }
            ToolEventData::Confirmed { .. }
            | ToolEventData::Rejected { .. }
            | ToolEventData::Cancelled { .. }
            | ToolEventData::Completed { .. }
            | ToolEventData::Failed { .. } => {
                self.push_clear(tool_id).await;
            }
            _ => {}
        }

        Ok(())
    }
}

/// Build a short hint string from tool params for the M5StickC display.
fn make_hint(params: &serde_json::Value) -> String {
    match params {
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (key, val) in map.iter().take(3) {
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                parts.push(format!("{}: {}", key, truncate_for_log(&val_str, 40)));
            }
            parts.join(", ")
        }
        _ => String::new(),
    }
}

/// Truncate a string to a fixed byte length on a UTF-8 boundary for logging.
fn truncate_for_log(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

/// Build a token usage query across all models/sessions for the given range.
fn token_query(time_range: TimeRange) -> TokenUsageQuery {
    TokenUsageQuery {
        model_id: None,
        session_id: None,
        time_range,
        limit: None,
        offset: None,
        include_subagent: false,
    }
}
