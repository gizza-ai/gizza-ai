//! gizza-ai/unix-timestamp-converter — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. No host calls —
//! runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_auto() -> String {
    "auto".to_string()
}

#[derive(Deserialize)]
struct Args {
    value: String,
    #[serde(default = "default_auto")]
    mode: String,
    #[serde(default = "default_auto")]
    unit: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("value").required().describe(
                "The thing to convert: a numeric Unix timestamp (e.g. 1700000000) to turn into a date, or a date/time string (e.g. '2023-11-14 22:13:20', '2023-11-14T22:13:20+02:00', 'January 1, 1970') to turn into a timestamp.",
            ),
        )
        .param(
            Param::enumv("mode", ["auto", "to-date", "to-timestamp"]).default("auto").describe(
                "Conversion direction. auto (default): a numeric value becomes a date, anything else is parsed as a date. to-date: force timestamp -> date. to-timestamp: force date -> timestamp.",
            ),
        )
        .param(
            Param::enumv("unit", ["auto", "seconds", "milliseconds", "microseconds", "nanoseconds"]).default("auto").describe(
                "Unit of the numeric timestamp when converting to a date (ignored for to-timestamp). auto (default) detects it from the magnitude. Outputs always include all four units.",
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
    name = "gizza-ai/unix-timestamp-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Unix timestamp converter skill",
    skill(
        description = "Convert between Unix (epoch) timestamps and human-readable dates in many common formats, entirely locally. Give 'value' as either a numeric timestamp (e.g. 1700000000) to convert to a UTC date, or a date/time string (ISO 8601 / RFC 3339, RFC 2822 email dates, US/European slash & dotted dates, month-name dates, e.g. '2023-11-14 22:13:20', '2023-11-14T22:13:20+02:00', 'January 1, 1970') to convert to a timestamp. 'mode' = auto (default; numeric -> date, else date -> timestamp), to-date, or to-timestamp. 'unit' = auto (default, detected from magnitude), seconds, milliseconds, microseconds, or nanoseconds — the unit of a numeric timestamp being read as a date (outputs always include all four units). For timestamp -> date it returns the timestamp in every unit, a UTC string, ISO 8601 and RFC 2822 renderings, and a full calendar breakdown (year, month + name, day, weekday, day-of-year, ISO week, hour/minute/second/nanosecond). For date -> timestamp it returns the Unix timestamp in seconds/milliseconds/microseconds/nanoseconds; an offset-less wall-clock is interpreted as UTC (assumed_utc=true). Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "unix-timestamp-converter", |a: Args| {
            gizza_ai_unix_timestamp_converter_core::run(&a.value, &a.mode, &a.unit)
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "value": { "type": "string", "description": "The thing to convert: a numeric Unix timestamp (e.g. 1700000000) to turn into a date, or a date/time string (e.g. '2023-11-14 22:13:20', '2023-11-14T22:13:20+02:00', 'January 1, 1970') to turn into a timestamp." },
                    "mode": { "type": "string", "enum": ["auto", "to-date", "to-timestamp"], "default": "auto", "description": "Conversion direction. auto (default): a numeric value becomes a date, anything else is parsed as a date. to-date: force timestamp -> date. to-timestamp: force date -> timestamp." },
                    "unit": { "type": "string", "enum": ["auto", "seconds", "milliseconds", "microseconds", "nanoseconds"], "default": "auto", "description": "Unit of the numeric timestamp when converting to a date (ignored for to-timestamp). auto (default) detects it from the magnitude. Outputs always include all four units." }
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
