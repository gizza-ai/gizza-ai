//! gizza-ai/clock — returns current UTC time as JSON.

use wafer_sdk::*;

struct Clock;

#[wafer_block(
    name = "gizza-ai/clock",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Current time skill",
    skill(
        description = "Get the current UTC time. Returns ISO 8601 timestamp.",
        parameters = r#"{
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }"#
    ),
)]
impl Clock {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        let now = chrono::Utc::now().to_rfc3339();
        let body = serde_json::json!({
            "time": now,
            "tz": "UTC",
        });
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        GuestResult::respond(bytes)
    }
}
