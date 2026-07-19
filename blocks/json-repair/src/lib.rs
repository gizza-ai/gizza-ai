//! gizza-ai/json-repair — repair malformed JSON (trailing commas, single
//! quotes, unquoted keys, missing commas, comments, Python literals, markdown
//! fences, truncated output) into valid JSON. Chat schema single-sourced from
//! descriptor(); handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_repair_core::repair;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_indent")]
    indent: String,
}

fn default_indent() -> String {
    "2".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe(
            "The broken/malformed JSON text to repair. Handles trailing commas, single or smart quotes, unquoted keys and values, missing commas, // and /* */ comments, Python literals (True/False/None), undefined/NaN/Infinity, raw newlines inside strings, markdown ```json fences, mismatched brackets, and truncated output. Example: {'name': 'John', age: 30,}",
        ))
        .param(
            Param::enumv("indent", ["2", "4", "tab", "minify"])
                .default("2")
                .describe("Output formatting: '2' or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line. Default 2."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-repair",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Repair malformed JSON into valid JSON",
    skill(
        description = "Repair broken or malformed JSON into valid JSON. Fixes trailing commas, single/smart quotes, unquoted keys and values, missing commas, // and /* */ comments, Python literals (True/False/None), undefined/NaN/Infinity (to null), raw newlines in strings, markdown ```json fences around LLM output, mismatched brackets, and truncated JSON (unclosed strings/arrays/objects are closed). Key order is preserved; duplicate keys keep the last value; nesting is capped at 200 levels. indent chooses output formatting: 2 or 4 spaces, tab, or minify. Deterministic and syntax-only (no LLM). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-repair", |a: Args| {
            repair(&a.json, &a.indent).map_err(SkillError::InvalidArgs)
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
                    "json": { "type": "string", "description": "The broken/malformed JSON text to repair. Handles trailing commas, single or smart quotes, unquoted keys and values, missing commas, // and /* */ comments, Python literals (True/False/None), undefined/NaN/Infinity, raw newlines inside strings, markdown ```json fences, mismatched brackets, and truncated output. Example: {'name': 'John', age: 30,}" },
                    "indent": { "type": "string", "enum": ["2", "4", "tab", "minify"], "default": "2", "description": "Output formatting: '2' or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line. Default 2." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
