//! gizza-ai/markdown-to-confluence — convert Markdown into Confluence markup.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-to-confluence-core`. The
//! chat schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "storage".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    panel_blockquotes: bool,
    #[serde(default)]
    heading_offset: u32,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Markdown document to convert to Confluence markup."),
        )
        .param(
            Param::enumv("format", ["storage", "wiki"])
                .default("storage")
                .describe("Output dialect. 'storage' (default) emits Confluence storage format — the XHTML-based markup the Cloud REST API consumes, with <ac:…> structured macros for code blocks and info/note/warning/tip panels. 'wiki' emits legacy wiki markup for the Data Center / Server 'Insert markup' dialog."),
        )
        .param(
            Param::boolean("panel_blockquotes")
                .default(true)
                .describe("When true (default), a blockquote whose first line starts 'Note:', 'Warning:', 'Info:' or 'Tip:' (case-insensitive) is converted to the matching Confluence panel macro and the prefix is stripped. When false, every blockquote stays a literal quote."),
        )
        .param(
            Param::integer("heading_offset")
                .default(0)
                .min(0.0)
                .max(5.0)
                .describe("Demote every heading by this many levels, 0-5. For example 1 turns a Markdown '#' from h1 into h2 — useful when pasting under an existing page title. Levels are capped at h6. Default 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-confluence",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Markdown into Confluence storage format or wiki markup.",
    skill(
        description = "Convert a Markdown document into Confluence markup. Choose format='storage' (default) for the XHTML-based storage format the Cloud REST API consumes — with <ac:structured-macro> code blocks (language tag preserved) and info/note/warning/tip panels — or format='wiki' for legacy Data Center / Server wiki markup. Supports headings h1-h6 (demote every heading with heading_offset 0-5, capped at h6), bold/italic/strikethrough/inline code, links and images, ordered/unordered and nested lists, fenced code with a language tag, GitHub pipe tables, thematic breaks, and blockquotes. When panel_blockquotes=true (default), a blockquote whose first line starts 'Note:'/'Warning:'/'Info:'/'Tip:' becomes the matching Confluence panel macro. Output is escaped so prose is safe by construction.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "markdown-to-confluence", |a: Args| {
            gizza_ai_markdown_to_confluence_core::convert(
                &a.input,
                &a.format,
                a.panel_blockquotes,
                a.heading_offset,
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
                    "input": { "type": "string", "description": "The Markdown document to convert to Confluence markup." },
                    "format": { "type": "string", "enum": ["storage", "wiki"], "default": "storage", "description": "Output dialect. 'storage' (default) emits Confluence storage format — the XHTML-based markup the Cloud REST API consumes, with <ac:…> structured macros for code blocks and info/note/warning/tip panels. 'wiki' emits legacy wiki markup for the Data Center / Server 'Insert markup' dialog." },
                    "panel_blockquotes": { "type": "boolean", "default": true, "description": "When true (default), a blockquote whose first line starts 'Note:', 'Warning:', 'Info:' or 'Tip:' (case-insensitive) is converted to the matching Confluence panel macro and the prefix is stripped. When false, every blockquote stays a literal quote." },
                    "heading_offset": { "type": "integer", "minimum": 0, "maximum": 5, "default": 0, "description": "Demote every heading by this many levels, 0-5. For example 1 turns a Markdown '#' from h1 into h2 — useful when pasting under an existing page title. Levels are capped at h6. Default 0." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
