//! gizza-ai/find-duplicate-lines — list lines appearing more than once, with
//! counts. Chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_find_duplicate_lines_core::find;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    trim: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text whose lines should be checked for duplicates."))
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare lines case-insensitively."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from each line before comparing."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FindDuplicateLines;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/find-duplicate-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List lines that appear more than once, with counts",
    skill(
        description = "List the lines that appear more than once in the text, with how many times each occurs (most frequent first). Set ignore_case=true to compare case-insensitively and trim=true to ignore surrounding whitespace. Returns total/unique line counts and the list of duplicates with their counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl FindDuplicateLines {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "find-duplicate-lines", |a: Args| {
            Ok::<_, SkillError>(find(&a.text, a.ignore_case, a.trim))
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
                    "text": { "type": "string", "description": "The text whose lines should be checked for duplicates." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare lines case-insensitively." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from each line before comparing." }
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
