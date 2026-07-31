//! gizza-ai/markdown-strip — removes Markdown formatting, leaving clean plain text.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-strip-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    links: String,
    #[serde(default)]
    images: String,
    #[serde(default)]
    keep_list_markers: bool,
    /// Tool default is true; serde's bool default is false, so single-source the
    /// `true` default here to match the descriptor's `.default(true)`.
    #[serde(default = "default_true")]
    collapse_blank_lines: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The Markdown text to strip to plain text."),
        )
        .param(
            Param::enumv("links", ["text", "url", "both"])
                .default("text")
                .describe("How to render a [label](url) link: 'text' (default) keeps the visible label and drops the URL; 'url' keeps the URL; 'both' keeps 'label (url)'."),
        )
        .param(
            Param::enumv("images", ["alt", "drop"])
                .default("alt")
                .describe("How to render a ![alt](url) image: 'alt' (default) keeps the alt text; 'drop' removes images entirely."),
        )
        .param(
            Param::boolean("keep_list_markers")
                .default(false)
                .describe("When true, keep list bullets ('- ') and ordered numbering ('1. '); when false (default), remove the markers and leave one item per line."),
        )
        .param(
            Param::boolean("collapse_blank_lines")
                .default(true)
                .describe("When true (default), separate blocks with a single newline (compact); when false, keep a blank line between blocks."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownStrip;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-strip",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip Markdown formatting to clean plain text.",
    skill(
        description = "Remove all Markdown formatting from text, leaving clean plain text. Strips heading '#' markers, bold/italic/strikethrough emphasis, blockquote '>' markers, horizontal rules, and code fences (the code content is kept). Links render per links='text' (default, keep the visible label), 'url' (keep the URL), or 'both' ('label (url)'). Images render per images='alt' (default, keep the alt text) or 'drop'. Set keep_list_markers=true to preserve '- '/'1. ' list markers (default removes them, one item per line). Tables become bare cell text, cells joined by spaces, one row per line. collapse_blank_lines=true (default) separates blocks with a single newline; false keeps a blank line between blocks.",
        parameters = schema_json()
    )
)]
impl MarkdownStrip {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "markdown-strip", |a: Args| {
            gizza_ai_markdown_strip_core::strip(
                &a.text,
                &a.links,
                &a.images,
                a.keep_list_markers,
                a.collapse_blank_lines,
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
                    "text": { "type": "string", "description": "The Markdown text to strip to plain text." },
                    "links": { "type": "string", "enum": ["text", "url", "both"], "default": "text", "description": "How to render a [label](url) link: 'text' (default) keeps the visible label and drops the URL; 'url' keeps the URL; 'both' keeps 'label (url)'." },
                    "images": { "type": "string", "enum": ["alt", "drop"], "default": "alt", "description": "How to render a ![alt](url) image: 'alt' (default) keeps the alt text; 'drop' removes images entirely." },
                    "keep_list_markers": { "type": "boolean", "default": false, "description": "When true, keep list bullets ('- ') and ordered numbering ('1. '); when false (default), remove the markers and leave one item per line." },
                    "collapse_blank_lines": { "type": "boolean", "default": true, "description": "When true (default), separate blocks with a single newline (compact); when false, keep a blank line between blocks." }
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
