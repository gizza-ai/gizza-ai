//! gizza-ai/outline-to-json — convert an indented text outline into nested JSON.
//!
//! Thin chat-skill wrapper around `gizza-ai-outline-to-json-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    outline: String,
    #[serde(default)]
    format: String,
    /// Columns a literal tab counts for; core clamps 1..=16 (0 → 4). Default 4.
    #[serde(default)]
    tab_size: u32,
    /// Pretty-print the JSON (default true).
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("outline")
                .required()
                .describe("The indented text outline. Each non-blank line is a node; leading whitespace (spaces or tabs) sets nesting — a more-indented line is a child of the nearest preceding less-indented line. Blank lines are ignored; the first line must not be indented."),
        )
        .param(
            Param::enumv("format", ["children", "nested"])
                .default("children")
                .describe("Output shape. 'children' (default) is an array of node objects [{\"text\":…,\"children\":[…]}] preserving order and duplicate siblings; 'nested' is an object keyed by node text {\"a\":{\"b\":{}}} (a leaf is {} and duplicate sibling keys keep the last)."),
        )
        .param(
            Param::integer("tab_size")
                .default(4)
                .min(1.0)
                .max(16.0)
                .describe("How many columns a literal tab counts for when measuring indentation, 1-16. Lets you mix tabs and spaces. Default 4."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print the JSON with 2-space indentation. Set false for compact single-line output. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OutlineToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/outline-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Outline to JSON skill",
    skill(
        description = "Convert an indented text outline into nested JSON. Each non-blank line becomes a node and its leading whitespace (spaces or tabs) sets the nesting: a more-indented line is a child of the nearest preceding less-indented line. format='children' (default) returns an array of {\"text\":…,\"children\":[…]} node objects, preserving order; format='nested' returns an object keyed by node text like {\"a\":{\"b\":{}}}. tab_size sets how many columns a tab counts for (default 4) so tabs and spaces can be mixed. Blank lines are ignored and the first line must be unindented. Set pretty=false for compact output.",
        parameters = schema_json()
    ),
)]
impl OutlineToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "outline-to-json", |a: Args| {
            gizza_ai_outline_to_json_core::convert(&a.outline, &a.format, a.tab_size, a.pretty)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "outline": { "type": "string", "description": "The indented text outline. Each non-blank line is a node; leading whitespace (spaces or tabs) sets nesting — a more-indented line is a child of the nearest preceding less-indented line. Blank lines are ignored; the first line must not be indented." },
                    "format": { "type": "string", "enum": ["children", "nested"], "default": "children", "description": "Output shape. 'children' (default) is an array of node objects [{\"text\":…,\"children\":[…]}] preserving order and duplicate siblings; 'nested' is an object keyed by node text {\"a\":{\"b\":{}}} (a leaf is {} and duplicate sibling keys keep the last)." },
                    "tab_size": { "type": "integer", "minimum": 1, "maximum": 16, "default": 4, "description": "How many columns a literal tab counts for when measuring indentation, 1-16. Lets you mix tabs and spaces. Default 4." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print the JSON with 2-space indentation. Set false for compact single-line output. Default true." }
                },
                "required": ["outline"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
