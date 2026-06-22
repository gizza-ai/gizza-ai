//! gizza-ai/count-line-frequency — count how often each line/value occurs and
//! rank them. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_count_line_frequency_core::count;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    trim: bool,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text whose lines/values to count (one per line)."))
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("When true (default), 'Apple' and 'apple' count separately; false groups them."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("When true (default), strip surrounding whitespace before counting. Blank lines are always skipped."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CountLineFrequency;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/count-line-frequency",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count and rank how often each line occurs",
    skill(
        description = "Count how often each line/value occurs in text and rank them from most to least frequent (like `sort | uniq -c | sort -rn`). Returns each distinct value with its count, plus the number of distinct values and total lines. case_sensitive (default true) and trim (default true) control grouping; blank lines are skipped. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CountLineFrequency {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "count-line-frequency", |a: Args| {
            Ok(count(&a.text, a.case_sensitive, a.trim))
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
                    "text": { "type": "string", "description": "The text whose lines/values to count (one per line)." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "When true (default), 'Apple' and 'apple' count separately; false groups them." },
                    "trim": { "type": "boolean", "default": true, "description": "When true (default), strip surrounding whitespace before counting. Blank lines are always skipped." }
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
