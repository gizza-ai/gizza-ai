//! gizza-ai/json-to-markdown — renders an arbitrary JSON document as readable,
//! nested Markdown.
//!
//! Thin chat-skill wrapper around `gizza-ai-json-to-markdown-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    /// Heading level for the top-level sections; core clamps to 1..=6 (0 → 1).
    #[serde(default)]
    heading_level: u32,
    #[serde(default)]
    array_mode: String,
    /// Sort object keys / table columns alphabetically (default false).
    #[serde(default)]
    sort_keys: bool,
    /// Nesting depth to expand before collapsing to a code block; clamped 1..=20.
    #[serde(default)]
    max_depth: u32,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to render as Markdown — an object, an array, or a scalar. Example: {\"title\":\"My Note\",\"tags\":[\"work\",\"idea\"]}."),
        )
        .param(
            Param::integer("heading_level")
                .default(2)
                .min(1.0)
                .max(gizza_ai_json_to_markdown_core::MAX_HEADING_LEVEL as f64)
                .describe("Heading level (1-6) for the top-level object/array sections; each nested level goes one deeper (capped at 6). Default 2 (uses '##', leaving '#' for a page title)."),
        )
        .param(
            Param::enumv("array_mode", ["auto", "table", "list"])
                .default("auto")
                .describe("How arrays of objects render. 'auto' (default): a uniform array of flat objects becomes a Markdown table, anything else becomes a nested bullet list. 'table': force a table (non-scalar cells become inline JSON). 'list': force a nested bullet list."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("When true, sort object keys and table columns alphabetically. Default false (keep the document's original key order)."),
        )
        .param(
            Param::integer("max_depth")
                .default(6)
                .min(1.0)
                .max(gizza_ai_json_to_markdown_core::MAX_MAX_DEPTH as f64)
                .describe("Maximum nesting depth (1-20) to expand as Markdown; a subtree deeper than this is emitted verbatim as a fenced JSON code block instead. Default 6."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render an arbitrary JSON document as readable, nested Markdown.",
    skill(
        description = "Render an arbitrary JSON document (e.g. a notes-app export or API response) as readable, nested Markdown. A top-level object's scalar keys become '- **key**: value' bullets and its nested objects/arrays become headings starting at heading_level (default 2). A uniform array of flat objects becomes a Markdown table; other arrays become nested bullet lists (set array_mode='table' or 'list' to force one). Set sort_keys=true to order keys alphabetically. Subtrees deeper than max_depth (default 6) collapse to a fenced JSON code block. Returns an error on invalid JSON.",
        parameters = schema_json()
    ),
)]
impl JsonToMarkdown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-markdown", |a: Args| {
            gizza_ai_json_to_markdown_core::to_markdown(
                &a.json,
                a.heading_level,
                &a.array_mode,
                a.sort_keys,
                a.max_depth,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON document to render as Markdown — an object, an array, or a scalar. Example: {\"title\":\"My Note\",\"tags\":[\"work\",\"idea\"]}." },
                    "heading_level": { "type": "integer", "minimum": 1, "maximum": 6, "default": 2, "description": "Heading level (1-6) for the top-level object/array sections; each nested level goes one deeper (capped at 6). Default 2 (uses '##', leaving '#' for a page title)." },
                    "array_mode": { "type": "string", "enum": ["auto", "table", "list"], "default": "auto", "description": "How arrays of objects render. 'auto' (default): a uniform array of flat objects becomes a Markdown table, anything else becomes a nested bullet list. 'table': force a table (non-scalar cells become inline JSON). 'list': force a nested bullet list." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "When true, sort object keys and table columns alphabetically. Default false (keep the document's original key order)." },
                    "max_depth": { "type": "integer", "minimum": 1, "maximum": 20, "default": 6, "description": "Maximum nesting depth (1-20) to expand as Markdown; a subtree deeper than this is emitted verbatim as a fenced JSON code block instead. Default 6." }
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
