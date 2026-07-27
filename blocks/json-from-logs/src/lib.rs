//! gizza-ai/json-from-logs — chat skill block on the shared tool abstraction.
//! Scans mixed log/console text for embedded JSON objects/arrays and extracts
//! each as a separately pretty-printed, validated block. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI + page);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default)]
    output: String,
}

fn default_indent() -> u64 {
    2
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The raw log or console text to scan. Embedded JSON objects/arrays are found anywhere in the text (e.g. a `state={...}` fragment on a log line); surrounding prose is ignored."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level for each extracted block (0-8). Use 0 to minify each block to one compact line. Default 2."),
        )
        .param(
            Param::enumv("output", ["blocks", "array"])
                .default("blocks")
                .describe("Output shape. 'blocks' (default) prints each extracted JSON block separately under a '// block N (line L)' header; 'array' wraps every extracted block into one pretty-printed JSON array."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-from-logs",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract each embedded JSON object/array from mixed log text, pretty-printed and validated.",
    skill(
        description = "Scan mixed log or console text and pull out every embedded JSON object/array, validating each with a strict JSON parser and pretty-printing it separately. Brace-matches balanced {…}/[…] runs anywhere in the text (e.g. a state={…} fragment on a log line), so surrounding prose and non-JSON braces are ignored; only runs that actually parse are kept and nested JSON isn't double-extracted. indent is spaces per level (0-8, default 2; 0 minifies each block). output='blocks' (default) prints each block under a '// block N (line L)' header; output='array' wraps all blocks into one JSON array. Errors if no valid JSON block is found. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-from-logs", |a: Args| {
            gizza_ai_json_from_logs_core::run(&a.text, a.indent as usize, &a.output)
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text":   { "type": "string", "description": "The raw log or console text to scan. Embedded JSON objects/arrays are found anywhere in the text (e.g. a `state={...}` fragment on a log line); surrounding prose is ignored." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level for each extracted block (0-8). Use 0 to minify each block to one compact line. Default 2." },
                    "output": { "type": "string", "enum": ["blocks", "array"], "default": "blocks", "description": "Output shape. 'blocks' (default) prints each extracted JSON block separately under a '// block N (line L)' header; 'array' wraps every extracted block into one pretty-printed JSON array." }
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
