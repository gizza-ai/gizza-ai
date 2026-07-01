//! gizza-ai/parse-datetime — chat skill block on the shared tool abstraction.
//! Parses a single date/time string in many common formats and returns its
//! structured components as JSON. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("input")
            .required()
            .describe("The date and/or time string to parse. Accepts ISO 8601 / RFC 3339 (2024-01-05, 2024-01-05T14:30:00, 2024-01-05T14:30:00Z, 2024-01-05T14:30:00+02:00, 2024-01-05 14:30), RFC 2822 email dates (Fri, 05 Jan 2024 14:30:00 +0000), US slash dates (01/05/2024, month-first; day-first when the first field is >12), European dotted dates (05.01.2024, day-first), year-first (2024/01/05), month-name dates (January 5, 2024; 5 Jan 2024; Jan 5, 2024 2:30 PM), and bare clock times (14:30, 2:30:15, 3pm, 9:05 AM)."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-datetime",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a date/time string in many formats into structured components.",
    skill(
        description = "Parse a single date and/or time string in many common formats and return its structured components as JSON: kind (date/time/datetime), year, month (number + English name), day, weekday name, day_of_year, iso_week (ISO-8601 week number), hour/minute/second (24-hour), utc_offset_seconds when a timezone is present, and a canonical iso8601 rendering. Accepts ISO 8601 / RFC 3339 (2024-01-05, 2024-01-05T14:30:00Z, 2024-01-05T14:30:00+02:00, '2024-01-05 14:30'), RFC 2822 email dates ('Fri, 05 Jan 2024 14:30:00 +0000'), US slash dates (01/05/2024, read month-first unless the first field is >12), European dotted dates (05.01.2024, day-first), year-first (2024/01/05), month-name dates ('January 5, 2024', '5 Jan 2024', 'Jan 5, 2024 2:30 PM'), and bare clock times (14:30, 2:30:15, 3pm, '9:05 AM'). Two-digit years map 00-69 to 2000s and 70-99 to 1900s. Returns an error for inputs it cannot recognize or that are not valid calendar dates/times. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-datetime", |a: Args| {
            gizza_ai_parse_datetime_core::run(&a.input).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The date and/or time string to parse. Accepts ISO 8601 / RFC 3339 (2024-01-05, 2024-01-05T14:30:00, 2024-01-05T14:30:00Z, 2024-01-05T14:30:00+02:00, 2024-01-05 14:30), RFC 2822 email dates (Fri, 05 Jan 2024 14:30:00 +0000), US slash dates (01/05/2024, month-first; day-first when the first field is >12), European dotted dates (05.01.2024, day-first), year-first (2024/01/05), month-name dates (January 5, 2024; 5 Jan 2024; Jan 5, 2024 2:30 PM), and bare clock times (14:30, 2:30:15, 3pm, 9:05 AM)." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
