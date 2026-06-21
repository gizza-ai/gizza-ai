//! gizza-ai/date-diff — chat skill block on the shared tool abstraction.
//!
//! Computes the duration between two dates/datetimes. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`, which shapes `{ "result": <DateDiff> }`
//! so the LLM sees the full structured breakdown (years/months/days/… + totals +
//! a human-readable `summary`). Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    start: String,
    end: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("start").required().describe(
                "The start date or datetime. Accepts YYYY-MM-DD, \
                 YYYY-MM-DDTHH:MM:SS, RFC-3339 (with Z/offset), and common \
                 variants like YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY.",
            ),
        )
        .param(
            Param::string("end").required().describe(
                "The end date or datetime, in the same accepted formats as `start`.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/date-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Duration between two dates",
    skill(
        description = "Compute the duration between two dates or datetimes. Returns a calendar breakdown (years, months, days, hours, minutes, seconds), flat totals (weeks, days, hours, minutes, seconds), and a human-readable summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "date-diff", |a: Args| {
            gizza_ai_date_diff_core::diff(&a.start, &a.end).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored manifest schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "start": { "type": "string", "description": "The start date or datetime. Accepts YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, RFC-3339 (with Z/offset), and common variants like YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY." },
                    "end": { "type": "string", "description": "The end date or datetime, in the same accepted formats as `start`." }
                },
                "required": ["start", "end"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
