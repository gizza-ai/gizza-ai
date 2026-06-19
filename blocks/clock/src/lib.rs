//! gizza-ai/clock — returns current UTC time as JSON.
//!
//! The time string is formatted by `gizza-ai-clock-core` (shared with the
//! standalone /tools/clock/ page). The chat schema is derived from
//! `descriptor()` (single source — shared shape across chat + CLI); clock takes
//! no parameters, so the descriptor is `Input::None` with no params. The
//! response shape is flat (`{ time, tz }`), so the handler builds it directly
//! rather than going through `run_skill`'s `{ result: … }` envelope. The
//! #[wafer_block] macro emits wasm-only registration; `build_response` is
//! testable on host. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, ToolDescriptor};
use wafer_sdk::*;

/// Single-source param descriptor → chat schema (and CLI). Clock takes no
/// parameters, so this is `Input::None` with no `.param()` calls. See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

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
        parameters = schema_json()
    ),
)]
impl Clock {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        // Flat response shape (`{ time, tz }`) — read directly by the LLM, so no
        // `{ result: … }` envelope. See block-utils "Response shapes" notes.
        let body = build_response(chrono::Utc::now());
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        GuestResult::respond(bytes)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

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

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. Clock's authored
    /// schema already had empty `properties` and `additionalProperties: false`,
    /// which is exactly what `Input::None` with no params emits — an exact match.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
