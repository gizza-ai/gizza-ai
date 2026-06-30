//! gizza-ai/toc-generator — build a linked table of contents from the headings of
//! a Markdown or HTML document.
//!
//! Thin chat-skill wrapper around `gizza-ai-toc-generator-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    document: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output_format: String,
    #[serde(default = "default_min")]
    min_level: u32,
    #[serde(default = "default_max")]
    max_level: u32,
    #[serde(default)]
    ordered: bool,
}

fn default_min() -> u32 {
    1
}
fn default_max() -> u32 {
    6
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("document")
                .required()
                .describe("The Markdown or HTML document to build a table of contents from. Headings (Markdown '#'…'######' / setext, or HTML <h1>…<h6>) become linked entries."),
        )
        .param(
            Param::enumv("input_format", ["auto", "markdown", "html"])
                .default("auto")
                .describe("How to read the document. 'auto' (default) detects HTML when an <h1>…<h6> tag is present, otherwise treats it as Markdown; 'markdown' or 'html' force the parser."),
        )
        .param(
            Param::enumv("output_format", ["markdown", "html"])
                .default("markdown")
                .describe("The table-of-contents format. 'markdown' (default) is a nested bullet/number list of [text](#anchor) links; 'html' is a nested <ul>/<ol> of <a href=\"#anchor\"> links."),
        )
        .param(
            Param::integer("min_level")
                .default(1)
                .min(1.0)
                .max(6.0)
                .describe("Shallowest heading level to include (1-6). Default 1. Use 2 to skip the document's single top <h1> title."),
        )
        .param(
            Param::integer("max_level")
                .default(6)
                .min(1.0)
                .max(6.0)
                .describe("Deepest heading level to include (1-6). Default 6. Lower it (e.g. 3) to keep the table of contents short."),
        )
        .param(
            Param::boolean("ordered")
                .default(false)
                .describe("Use a numbered list (Markdown '1.' / HTML <ol>) instead of bullets (Markdown '-' / HTML <ul>). Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TocGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/toc-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Table-of-contents generator skill",
    skill(
        description = "Build a linked table of contents from the headings of a Markdown or HTML document. Reads ATX ('#'…'######') and setext Markdown headings or HTML <h1>…<h6> tags (input_format='auto' detects which, or force 'markdown'/'html'), and emits a nested list of links to GitHub-style heading anchors. output_format='markdown' (default) returns a nested [text](#anchor) bullet list; 'html' returns a nested <ul>/<ol> of <a href=\"#anchor\"> links. min_level/max_level (1-6) limit which heading levels appear; set ordered=true for a numbered list. HTML headings with an existing id keep that id as the anchor; duplicate headings get unique -1, -2 suffixes. Pass the document as `document`.",
        parameters = schema_json()
    ),
)]
impl TocGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "toc-generator", |a: Args| {
            gizza_ai_toc_generator_core::generate(
                &a.document,
                &a.input_format,
                &a.output_format,
                a.min_level,
                a.max_level,
                a.ordered,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "document": { "type": "string", "description": "The Markdown or HTML document to build a table of contents from. Headings (Markdown '#'…'######' / setext, or HTML <h1>…<h6>) become linked entries." },
                    "input_format": { "type": "string", "enum": ["auto", "markdown", "html"], "default": "auto", "description": "How to read the document. 'auto' (default) detects HTML when an <h1>…<h6> tag is present, otherwise treats it as Markdown; 'markdown' or 'html' force the parser." },
                    "output_format": { "type": "string", "enum": ["markdown", "html"], "default": "markdown", "description": "The table-of-contents format. 'markdown' (default) is a nested bullet/number list of [text](#anchor) links; 'html' is a nested <ul>/<ol> of <a href=\"#anchor\"> links." },
                    "min_level": { "type": "integer", "minimum": 1, "maximum": 6, "default": 1, "description": "Shallowest heading level to include (1-6). Default 1. Use 2 to skip the document's single top <h1> title." },
                    "max_level": { "type": "integer", "minimum": 1, "maximum": 6, "default": 6, "description": "Deepest heading level to include (1-6). Default 6. Lower it (e.g. 3) to keep the table of contents short." },
                    "ordered": { "type": "boolean", "default": false, "description": "Use a numbered list (Markdown '1.' / HTML <ol>) instead of bullets (Markdown '-' / HTML <ul>). Default false." }
                },
                "required": ["document"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
