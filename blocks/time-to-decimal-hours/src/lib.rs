//! gizza-ai/time-to-decimal-hours — chat skill block on the shared tool abstraction.
//!
//! Converts a clock duration (`HH:MM` / `HH:MM:SS`) into decimal hours and back.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`, which shapes
//! `{ "result": <Conversion> }` so the LLM sees both the clock form and the
//! decimal form plus flat totals. Pure compute — no host capabilities used.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    value: String,
    #[serde(default)]
    mode: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("value").required().describe(
                "The duration to convert. Either a clock duration like '1:30' or \
                 '01:30:45' (H:MM or H:MM:SS), or a decimal number of hours like \
                 '1.5'. A leading '-' makes it negative.",
            ),
        )
        .param(
            Param::enumv("mode", ["auto", "from-clock", "from-decimal"])
                .default("auto")
                .describe(
                    "How to interpret `value`. 'auto' (default) reads it as a \
                     clock duration if it contains a colon, else as decimal \
                     hours; 'from-clock' forces clock parsing; 'from-decimal' \
                     forces decimal-hours parsing.",
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
    name = "gizza-ai/time-to-decimal-hours",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert HH:MM clock durations to decimal hours and back",
    skill(
        description = "Convert a clock duration (H:MM or H:MM:SS) into decimal hours (e.g. 1:30 → 1.5) and convert decimal hours back into a clock duration (e.g. 1.5 → 1:30). Returns both forms together plus flat totals in minutes and seconds. Use 'auto' to detect the input form, or force it with 'from-clock' / 'from-decimal'. Handy for timesheets, payroll and billing.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "time-to-decimal-hours", |a: Args| {
            gizza_ai_time_to_decimal_hours_core::convert(&a.value, &a.mode)
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

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored manifest schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "value": { "type": "string", "description": "The duration to convert. Either a clock duration like '1:30' or '01:30:45' (H:MM or H:MM:SS), or a decimal number of hours like '1.5'. A leading '-' makes it negative." },
                    "mode": { "type": "string", "enum": ["auto", "from-clock", "from-decimal"], "default": "auto", "description": "How to interpret `value`. 'auto' (default) reads it as a clock duration if it contains a colon, else as decimal hours; 'from-clock' forces clock parsing; 'from-decimal' forces decimal-hours parsing." }
                },
                "required": ["value"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
