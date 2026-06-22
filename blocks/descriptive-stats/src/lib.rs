//! gizza-ai/descriptive-stats — descriptive statistics for a list of numbers.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_descriptive_stats_core::describe;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    numbers: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("numbers")
            .required()
            .describe("The numbers to summarize, separated by spaces, commas, semicolons, or newlines."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DescriptiveStats;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/descriptive-stats",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Descriptive statistics for a list of numbers",
    skill(
        description = "Compute descriptive statistics for a list of numbers (separated by spaces, commas, semicolons, or newlines): count, sum, mean, median, mode (most frequent value(s)), min, max, range, quartiles Q1/Q3 and IQR (numpy-linear), and both population and sample variance & standard deviation. Runs locally.",
        parameters = schema_json()
    ),
)]
impl DescriptiveStats {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "descriptive-stats", |a: Args| {
            describe(&a.numbers).map_err(SkillError::InvalidArgs)
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
                    "numbers": { "type": "string", "description": "The numbers to summarize, separated by spaces, commas, semicolons, or newlines." }
                },
                "required": ["numbers"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
