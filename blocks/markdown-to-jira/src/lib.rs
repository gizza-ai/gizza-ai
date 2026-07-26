//! gizza-ai/markdown-to-jira — bidirectional Markdown ↔ Jira wiki markup converter.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-to-jira-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    heading_offset: i64,
    #[serde(default = "default_true")]
    panel_blockquotes: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Markdown or Jira wiki markup text to convert."),
        )
        .param(
            Param::enumv("direction", ["md-to-jira", "jira-to-md"])
                .default("md-to-jira")
                .describe("Conversion direction. 'md-to-jira' converts Markdown into Jira wiki markup; 'jira-to-md' converts Jira wiki markup back to Markdown."),
        )
        .param(
            Param::integer("heading_offset")
                .default(0.0)
                .min(0.0)
                .max(5.0)
                .describe("Markdown-to-Jira only: demote Markdown headings by this many levels before emitting h1. through h6. Use 1 when pasting under an existing page title. Range 0-5."),
        )
        .param(
            Param::boolean("panel_blockquotes")
                .default(true)
                .describe("When true (default), Markdown blockquotes starting with Note:, Info:, Warning:, or Tip: become Jira panel macros, and those Jira macros become Markdown blockquotes on the reverse path."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownToJira;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-jira",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Markdown and Jira wiki markup both ways.",
    skill(
        description = "Convert Markdown into Jira wiki markup, or Jira wiki markup back to Markdown. Handles headings, bold/italic/strike, inline code, fenced code blocks, links, images, ordered and unordered lists, tables, blockquotes, horizontal rules, and optional Note/Info/Warning/Tip panel macros. direction='md-to-jira' by default; set direction='jira-to-md' for reverse conversion. heading_offset (0-5) demotes Markdown headings when pasting under an existing title. Runs fully in the sandbox and does not produce Atlassian Document Format JSON.",
        parameters = schema_json()
    ),
)]
impl MarkdownToJira {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-to-jira", |a: Args| {
            gizza_ai_markdown_to_jira_core::convert(
                &a.input,
                &a.direction,
                a.heading_offset,
                a.panel_blockquotes,
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
    /// schema, so any future LLM-facing API change is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Markdown or Jira wiki markup text to convert." },
                    "direction": { "type": "string", "enum": ["md-to-jira", "jira-to-md"], "default": "md-to-jira", "description": "Conversion direction. 'md-to-jira' converts Markdown into Jira wiki markup; 'jira-to-md' converts Jira wiki markup back to Markdown." },
                    "heading_offset": { "type": "integer", "minimum": 0, "maximum": 5, "default": 0.0, "description": "Markdown-to-Jira only: demote Markdown headings by this many levels before emitting h1. through h6. Use 1 when pasting under an existing page title. Range 0-5." },
                    "panel_blockquotes": { "type": "boolean", "default": true, "description": "When true (default), Markdown blockquotes starting with Note:, Info:, Warning:, or Tip: become Jira panel macros, and those Jira macros become Markdown blockquotes on the reverse path." }
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
