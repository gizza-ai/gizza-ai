//! gizza-ai/html-diff — diff the visible text of two HTML snippets.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_granularity")]
    granularity: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default = "default_context")]
    context: u32,
}

fn default_granularity() -> String {
    "line".to_string()
}
fn default_format() -> String {
    "unified".to_string()
}
fn default_context() -> u32 {
    3
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("left").required().describe("The original/left HTML to compare."))
        .param(Param::string("right").required().describe("The updated/right HTML to compare."))
        .param(Param::enumv("granularity", ["line", "word"]).default("line").describe("Diff granularity: 'line' for patch-style line diffs or 'word' for inline word-level diffs."))
        .param(Param::enumv("format", ["unified", "json"]).default("unified").describe("Output format: unified diff/inline text or a structured JSON report."))
        .param(Param::boolean("ignore_case").default(false).describe("Compare tokens case-insensitively while preserving original text in the output."))
        .param(Param::boolean("ignore_whitespace").default(false).describe("Normalize runs of whitespace for matching while preserving original text in the output."))
        .param(Param::integer("context").default(3).min(0.0).max(100.0).describe("Unchanged context lines around each hunk in unified line output. Default 3."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff the visible text of two HTML snippets, ignoring tag and attribute noise",
    skill(
        description = "Compare two HTML snippets by their visible text only — every tag and attribute is stripped first, so markup differences (classes, ids, inline styles, wrapper elements) are ignored. Diff line-by-line for a classic unified patch or word-by-word for an inline wdiff-style view, and render as text or a structured JSON report with added/removed/changed/unchanged counts. Supports optional case-insensitive and whitespace-normalized matching while preserving original text in the output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-diff", |a: Args| {
            gizza_ai_html_diff_core::run(
                &a.left,
                &a.right,
                &a.granularity,
                &a.format,
                a.ignore_case,
                a.ignore_whitespace,
                a.context as usize,
            )
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "left": { "type": "string", "description": "The original/left HTML to compare." },
                "right": { "type": "string", "description": "The updated/right HTML to compare." },
                "granularity": { "type": "string", "enum": ["line", "word"], "default": "line", "description": "Diff granularity: 'line' for patch-style line diffs or 'word' for inline word-level diffs." },
                "format": { "type": "string", "enum": ["unified", "json"], "default": "unified", "description": "Output format: unified diff/inline text or a structured JSON report." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Compare tokens case-insensitively while preserving original text in the output." },
                "ignore_whitespace": { "type": "boolean", "default": false, "description": "Normalize runs of whitespace for matching while preserving original text in the output." },
                "context": { "type": "integer", "minimum": 0, "maximum": 100, "default": 3, "description": "Unchanged context lines around each hunk in unified line output. Default 3." }
            },
            "required": ["left", "right"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
