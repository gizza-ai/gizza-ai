//! gizza-ai/markdown-section-extractor — extract a single section from a Markdown
//! document by its heading.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-section-extractor-core`. The
//! chat schema is single-sourced from `descriptor()` (also drives the CLI + page
//! query-params); `handle()` delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    heading: String,
    #[serde(default)]
    match_mode: String,
    /// Include deeper-nested subsections in the result (default true).
    #[serde(default = "default_true")]
    include_subsections: bool,
    /// Include the matched heading line itself in the result (default true).
    #[serde(default = "default_true")]
    include_heading: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The full Markdown document to extract a section from."),
        )
        .param(
            Param::string("heading")
                .required()
                .describe("The heading text of the section to extract (without the leading '#'), e.g. 'Installation'."),
        )
        .param(
            Param::enumv("match_mode", ["exact", "exact_case", "contains"])
                .default("exact")
                .describe("How to match the heading: 'exact' (default) is a case-insensitive exact match of the trimmed heading text; 'exact_case' is case-sensitive; 'contains' matches any heading whose text contains the query (case-insensitive). The first matching heading wins."),
        )
        .param(
            Param::boolean("include_subsections")
                .default(true)
                .describe("When true (default), the extracted section includes every deeper-nested subsection, ending at the next heading of the same or a shallower level. When false, it stops at the first heading of any level after the target, returning only the body directly under it."),
        )
        .param(
            Param::boolean("include_heading")
                .default(true)
                .describe("When true (default), the matched heading line is the first line of the output. When false, only the section body is returned."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-section-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extracts a single subsection from a Markdown document by its heading.",
    skill(
        description = "Extract a single subsection from a Markdown document by its heading. Give the full document as 'markdown' and the heading text as 'heading' (e.g. 'Installation', without the '#'). Returns the heading and everything beneath it up to the next heading at the same or a shallower level — by default including all deeper nested subsections. Set include_subsections=false to return only the body directly under the heading (stopping at the first deeper heading). Set include_heading=false to omit the heading line. match_mode controls how the heading is found: 'exact' (default, case-insensitive), 'exact_case' (case-sensitive), or 'contains' (substring). Recognizes ATX (#, ##, …) and setext (===/---) headings and skips '#' lines inside fenced code blocks.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-section-extractor", |a: Args| {
            gizza_ai_markdown_section_extractor_core::extract(
                &a.markdown,
                &a.heading,
                &a.match_mode,
                a.include_subsections,
                a.include_heading,
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
                    "markdown": { "type": "string", "description": "The full Markdown document to extract a section from." },
                    "heading": { "type": "string", "description": "The heading text of the section to extract (without the leading '#'), e.g. 'Installation'." },
                    "match_mode": { "type": "string", "enum": ["exact", "exact_case", "contains"], "default": "exact", "description": "How to match the heading: 'exact' (default) is a case-insensitive exact match of the trimmed heading text; 'exact_case' is case-sensitive; 'contains' matches any heading whose text contains the query (case-insensitive). The first matching heading wins." },
                    "include_subsections": { "type": "boolean", "default": true, "description": "When true (default), the extracted section includes every deeper-nested subsection, ending at the next heading of the same or a shallower level. When false, it stops at the first heading of any level after the target, returning only the body directly under it." },
                    "include_heading": { "type": "boolean", "default": true, "description": "When true (default), the matched heading line is the first line of the output. When false, only the section body is returned." }
                },
                "required": ["markdown", "heading"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
