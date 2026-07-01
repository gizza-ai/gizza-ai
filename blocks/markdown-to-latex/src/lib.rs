//! gizza-ai/markdown-to-latex — convert a Markdown document into LaTeX source.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-to-latex-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    /// Wrap the output in a standalone \documentclass document (default false).
    #[serde(default)]
    full_document: bool,
    /// Shift every heading down by this many levels, 0-5 (default 0).
    #[serde(default)]
    heading_offset: u32,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown document to convert to LaTeX."),
        )
        .param(
            Param::boolean("full_document")
                .default(false)
                .describe("When true, wrap the LaTeX body in a compilable standalone document (\\documentclass{article} preamble with the needed packages, plus \\begin{document}/\\end{document}). When false (default), emit only the body fragment to paste into an existing document."),
        )
        .param(
            Param::integer("heading_offset")
                .default(0)
                .min(0.0)
                .max(5.0)
                .describe("Shift every heading down by this many levels, 0-5. For example 1 turns a Markdown '#' from \\section into \\subsection — useful when pasting into an existing chapter. Default 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-latex",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Markdown document into LaTeX source.",
    skill(
        description = "Convert a Markdown document into LaTeX source. Supports ATX and setext headings (mapped to \\section/\\subsection/...), ordered/unordered and nested lists, GitHub task lists, fenced and indented code blocks (via the listings/verbatim environments), blockquotes, GitHub pipe tables (tabular with booktabs rules and column alignment), thematic breaks, links and images (hyperref/graphicx), autolinks, inline bold/italic/code, strikethrough (~~text~~ via ulem \\sout), and footnotes ([^id] references with [^id]: definitions). The ten LaTeX special characters in prose are escaped automatically, and inline/display math ($...$ and $$...$$) is passed through verbatim. Set full_document=true to wrap the body in a compilable standalone document, or heading_offset=N (0-5) to demote headings when pasting into an existing document.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "markdown-to-latex", |a: Args| {
            gizza_ai_markdown_to_latex_core::convert(&a.markdown, a.full_document, a.heading_offset)
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
                    "markdown": { "type": "string", "description": "The Markdown document to convert to LaTeX." },
                    "full_document": { "type": "boolean", "default": false, "description": "When true, wrap the LaTeX body in a compilable standalone document (\\documentclass{article} preamble with the needed packages, plus \\begin{document}/\\end{document}). When false (default), emit only the body fragment to paste into an existing document." },
                    "heading_offset": { "type": "integer", "minimum": 0, "maximum": 5, "default": 0, "description": "Shift every heading down by this many levels, 0-5. For example 1 turns a Markdown '#' from \\section into \\subsection — useful when pasting into an existing chapter. Default 0." }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
