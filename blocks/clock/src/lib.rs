//! gizza-ai/clock — returns current UTC time as JSON.
//!
//! The time string is formatted by `gizza-ai-clock-core` (shared with the
//! standalone /tools/clock/ page). The #[wafer_block] macro emits wasm-only
//! registration; `build_response` is testable on host.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use wafer_sdk::*;

/// Build the JSON response body for a clock reading. Extracted so host tests can
/// pin the shape against a known timestamp.
fn build_response(now: chrono::DateTime<chrono::Utc>) -> serde_json::Value {
    serde_json::json!({
        "time": gizza_ai_clock_core::format_rfc3339(now),
        "tz": "UTC",
    })
}

#[cfg(target_arch = "wasm32")]
struct Clock;

#[cfg(target_arch = "wasm32")]
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
        let body = build_response(chrono::Utc::now());
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        GuestResult::respond(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn response_pins_iso_timestamp_and_utc_tz() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap();
        let v = build_response(now);
        assert_eq!(v["time"], "2026-04-20T12:00:00+00:00");
        assert_eq!(v["tz"], "UTC");
    }

    #[test]
    fn response_serializes_to_compact_json() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let v = build_response(now);
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains(r#""time":"2026-01-01T00:00:00+00:00""#));
        assert!(s.contains(r#""tz":"UTC""#));
    }
}
