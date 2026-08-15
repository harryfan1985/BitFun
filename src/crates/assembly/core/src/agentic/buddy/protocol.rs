//! Hardware Buddy BLE wire protocol (newline-delimited JSON over NUS).
//!
//! The device firmware consumes a heartbeat snapshot and emits permission
//! decisions; the host also sends time sync and clear commands. See the
//! claude-desktop-buddy REFERENCE.md for the full wire contract.

use serde_json::{json, Value};

/// Heartbeat fields the device displays.
#[derive(Debug, Clone, Default)]
pub struct Heartbeat {
    pub total: u32,
    pub running: u32,
    pub waiting: u32,
    pub msg: String,
    pub tokens: u64,
    pub tokens_today: u64,
    pub prompt_id: Option<String>,
    pub prompt_tool: String,
    pub prompt_hint: String,
}

/// A permission decision received from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    pub id: String,
    pub decision: PermissionChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChoice {
    Once,
    Deny,
}

/// Encode a heartbeat snapshot as one newline-delimited JSON line.
pub fn encode_heartbeat(heartbeat: &Heartbeat) -> String {
    let prompt = heartbeat.prompt_id.as_ref().map(|id| {
        json!({
            "id": id,
            "tool": heartbeat.prompt_tool,
            "hint": heartbeat.prompt_hint,
        })
    });

    let mut body = json!({
        "total": heartbeat.total,
        "running": heartbeat.running,
        "waiting": heartbeat.waiting,
        "msg": heartbeat.msg,
        "tokens": heartbeat.tokens,
        "tokens_today": heartbeat.tokens_today,
    });
    if let Some(prompt) = prompt {
        body["prompt"] = prompt;
    }
    body.to_string()
}

/// Encode a clear-screen command for a resolved prompt.
pub fn encode_clear(prompt_id: &str) -> String {
    json!({ "prompt_id": prompt_id }).to_string()
}

/// Encode a time sync command. `offset_seconds` is east of UTC (e.g. +28800
/// for UTC+8).
pub fn encode_time_sync(epoch_seconds: i64, offset_seconds: i32) -> String {
    json!({ "time": [epoch_seconds, offset_seconds] }).to_string()
}

/// Decode a permission decision line from the device.
///
/// Returns `None` for non-permission messages (acks, status, etc.).
pub fn decode_permission(line: &str) -> Option<PermissionDecision> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("cmd")?.as_str()? != "permission" {
        return None;
    }
    let id = value.get("id")?.as_str()?.to_string();
    let decision = match value.get("decision")?.as_str()? {
        "once" => PermissionChoice::Once,
        "deny" => PermissionChoice::Deny,
        _ => return None,
    };
    Some(PermissionDecision { id, decision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_with_prompt_round_trips_fields() {
        let line = encode_heartbeat(&Heartbeat {
            total: 3,
            running: 1,
            waiting: 1,
            msg: "approve: Bash".into(),
            tokens: 100,
            tokens_today: 20,
            prompt_id: Some("tool_1".into()),
            prompt_tool: "Bash".into(),
            prompt_hint: "rm -rf /tmp".into(),
        });
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(v["total"], 3);
        assert_eq!(v["waiting"], 1);
        assert_eq!(v["msg"], "approve: Bash");
        assert_eq!(v["prompt"]["id"], "tool_1");
    }

    #[test]
    fn heartbeat_without_prompt_omits_field() {
        let line = encode_heartbeat(&Heartbeat::default());
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert!(v.get("prompt").is_none());
    }

    #[test]
    fn decode_permission_once() {
        let d = decode_permission(r#"{"cmd":"permission","id":"p","decision":"once"}"#)
            .expect("permission");
        assert_eq!(d.id, "p");
        assert_eq!(d.decision, PermissionChoice::Once);
    }

    #[test]
    fn decode_permission_deny() {
        let d = decode_permission(r#"{"cmd":"permission","id":"p","decision":"deny"}"#)
            .expect("permission");
        assert_eq!(d.decision, PermissionChoice::Deny);
    }

    #[test]
    fn decode_non_permission_is_none() {
        assert!(decode_permission(r#"{"ack":"status","ok":true}"#).is_none());
        assert!(decode_permission("not json").is_none());
    }

    #[test]
    fn time_sync_format() {
        let line = encode_time_sync(1786719467, 28800);
        let v: Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(v["time"][0], 1786719467);
        assert_eq!(v["time"][1], 28800);
    }
}
