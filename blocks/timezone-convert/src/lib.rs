//! gizza-ai/timezone-convert — convert a date/time from one IANA timezone to
//! another, with correct daylight-saving handling.
//!
//! Thin chat-skill wrapper around `gizza-ai-timezone-convert-core`. The chat
//! schema is single-sourced from `descriptor()` (shared with the CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    datetime: String,
    from: String,
    to: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("datetime")
                .required()
                .describe("The wall-clock date/time to convert, in the 'from' zone. ISO form, e.g. '2024-03-10 14:30' or '2024-03-10T14:30:00' (date-only assumes midnight). Do NOT include a 'Z' or offset — the zone is given by 'from'."),
        )
        .param(
            Param::string("from")
                .required()
                .describe("Source IANA timezone name, e.g. 'America/New_York', 'Europe/London', 'Asia/Tokyo', or 'UTC'."),
        )
        .param(
            Param::string("to")
                .required()
                .describe("Target IANA timezone name, e.g. 'Asia/Kolkata', 'Australia/Sydney', or 'UTC'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/timezone-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a date/time between timezones",
    skill(
        description = "Convert a wall-clock date/time from one IANA timezone to another, correctly handling daylight saving time (DST). Give 'datetime' as a plain ISO time like '2024-03-10 14:30' (interpreted in the 'from' zone, no trailing Z/offset), and 'from'/'to' as IANA zone names like 'America/New_York', 'Europe/London', 'Asia/Tokyo', 'Asia/Kolkata', or 'UTC'. Returns the converted time (ISO 8601 with offset) plus the offsets, the difference in hours/minutes, the target weekday, whether the target is in DST, and the Unix timestamp. A time that falls in a spring-forward DST gap (which never occurs) is rejected.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "timezone-convert", |a: Args| {
            gizza_ai_timezone_convert_core::render(&a.datetime, &a.from, &a.to)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "datetime": { "type": "string", "description": "The wall-clock date/time to convert, in the 'from' zone. ISO form, e.g. '2024-03-10 14:30' or '2024-03-10T14:30:00' (date-only assumes midnight). Do NOT include a 'Z' or offset — the zone is given by 'from'." },
                    "from": { "type": "string", "description": "Source IANA timezone name, e.g. 'America/New_York', 'Europe/London', 'Asia/Tokyo', or 'UTC'." },
                    "to": { "type": "string", "description": "Target IANA timezone name, e.g. 'Asia/Kolkata', 'Australia/Sydney', or 'UTC'." }
                },
                "required": ["datetime", "from", "to"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
