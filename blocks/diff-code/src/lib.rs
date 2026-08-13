//! gizza-ai/diff-code — compare two code/text snippets side by side with
//! word- or character-level intra-line highlighting.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_view")]
    view: String,
    #[serde(default = "default_granularity")]
    granularity: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default = "default_context")]
    context: u32,
    #[serde(default = "default_line_numbers")]
    line_numbers: bool,
    #[serde(default = "default_width")]
    width: u32,
}

fn default_view() -> String { "side-by-side".to_string() }
fn default_granularity() -> String { "word".to_string() }
fn default_context() -> u32 { 3 }
fn default_line_numbers() -> bool { true }
fn default_width() -> u32 { 60 }

/// Single source for the chat schema and the CLI.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The original/left code or text, newline-separated. Max 1 MB. Example: \"fn main() {\\n    println!(\\\"hi\\\");\\n}\"."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The updated/right code or text to compare against left, newline-separated. Max 1 MB."),
        )
        .param(
            Param::enumv("view", ["side-by-side", "unified", "word-diff", "stats", "json"])
                .default("side-by-side")
                .describe("How to render the diff: 'side-by-side' two aligned columns (default), 'unified' a clean @@ patch you can apply, 'word-diff' one merged stream using [-removed-]/{+added+}, 'stats' counts and similarity, 'json' a structured report with rows and spans."),
        )
        .param(
            Param::enumv("granularity", ["word", "char", "none"])
                .default("word")
                .describe("Intra-line refinement for a changed line pair: 'word' marks changed tokens (default), 'char' marks individual characters, 'none' marks the whole line. Lines over 400 tokens per side fall back to whole-line marking."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare lines case-insensitively. Matching only — the output always echoes the original text. Default false."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe("Collapse runs of whitespace and trim line ends before comparing. Matching only — the output always echoes the original text. Default false."),
        )
        .param(
            Param::integer("context")
                .default(3)
                .min(0.0)
                .max(100.0)
                .describe("Unchanged lines kept around each change; further runs collapse into a '… N unchanged lines …' marker. 0 shows only changes. Default 3."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(true)
                .describe("Show per-side line numbers in the side-by-side and word-diff views. Ignored by 'unified', 'stats', and 'json'. Default true."),
        )
        .param(
            Param::integer("width")
                .default(60)
                .min(20.0)
                .max(200.0)
                .describe("Per-column content width in characters for the side-by-side view; tabs expand to 4 spaces for alignment. Default 60."),
        )
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diff-code",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two code snippets side by side with word-level highlighting",
    skill(
        description = "Compare two code or text snippets and render the difference as side-by-side columns, a clean unified patch, a merged word-diff stream, a stat summary, or structured JSON. Uses a patience diff so repeated braces and blank lines stay aligned, then refines each changed line pair at word or character level and marks it with the git word-diff convention [-removed-]/{+added+}. Optional case-insensitive and whitespace-insensitive matching never alters the text that is echoed back. Context lines collapse unchanged regions. Limit 1 MB per side.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "diff-code", |a: Args| {
            gizza_ai_diff_code_core::run(
                &a.left,
                &a.right,
                &a.view,
                &a.granularity,
                a.ignore_case,
                a.ignore_whitespace,
                a.context as usize,
                a.line_numbers,
                a.width as usize,
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
                "left": { "type": "string", "description": "The original/left code or text, newline-separated. Max 1 MB. Example: \"fn main() {\\n    println!(\\\"hi\\\");\\n}\"." },
                "right": { "type": "string", "description": "The updated/right code or text to compare against left, newline-separated. Max 1 MB." },
                "view": { "type": "string", "enum": ["side-by-side", "unified", "word-diff", "stats", "json"], "default": "side-by-side", "description": "How to render the diff: 'side-by-side' two aligned columns (default), 'unified' a clean @@ patch you can apply, 'word-diff' one merged stream using [-removed-]/{+added+}, 'stats' counts and similarity, 'json' a structured report with rows and spans." },
                "granularity": { "type": "string", "enum": ["word", "char", "none"], "default": "word", "description": "Intra-line refinement for a changed line pair: 'word' marks changed tokens (default), 'char' marks individual characters, 'none' marks the whole line. Lines over 400 tokens per side fall back to whole-line marking." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Compare lines case-insensitively. Matching only — the output always echoes the original text. Default false." },
                "ignore_whitespace": { "type": "boolean", "default": false, "description": "Collapse runs of whitespace and trim line ends before comparing. Matching only — the output always echoes the original text. Default false." },
                "context": { "type": "integer", "minimum": 0, "maximum": 100, "default": 3, "description": "Unchanged lines kept around each change; further runs collapse into a '… N unchanged lines …' marker. 0 shows only changes. Default 3." },
                "line_numbers": { "type": "boolean", "default": true, "description": "Show per-side line numbers in the side-by-side and word-diff views. Ignored by 'unified', 'stats', and 'json'. Default true." },
                "width": { "type": "integer", "minimum": 20, "maximum": 200, "default": 60, "description": "Per-column content width in characters for the side-by-side view; tabs expand to 4 spaces for alignment. Default 60." }
            },
            "required": ["left", "right"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_descriptor_view_is_a_core_view() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let views: Vec<String> = schema["properties"]["view"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(views, gizza_ai_diff_code_core::VIEWS.to_vec());
    }
}
