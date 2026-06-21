//! gizza-ai/extract-dates — scan text for date/time mentions, normalized to ISO
//! 8601. Chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_dates_core::extract;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

#[derive(Serialize)]
struct Resp {
    count: usize,
    dates: Vec<gizza_ai_extract_dates_core::Found>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("The text to scan for dates and times."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractDates;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-dates",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find dates and times in text, normalized to ISO 8601",
    skill(
        description = "Scan a block of text and list every date and time mention, each normalized to ISO 8601. Recognizes ISO dates/datetimes (2024-01-05, 2024-01-05T14:30), numeric dates (01/05/2024 — read month-first/US unless the first field is >12), year-first dates (2024/01/05), month-name dates ('January 5, 2024', '5 Jan 2024'), and clock times (14:30, 3pm, 9:05 AM). Returns each match's original text, its ISO value, and its kind (date/datetime/time), in order of appearance. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractDates {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-dates", |a: Args| {
            let dates = extract(&a.text);
            Ok::<Resp, SkillError>(Resp { count: dates.len(), dates })
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to scan for dates and times." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
