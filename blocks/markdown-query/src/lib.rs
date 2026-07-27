//! gizza-ai/markdown-query — chat skill block on the shared tool abstraction.
//! "jq for Markdown": pull headings, links, images, code blocks, or tables out
//! of a Markdown document and render them as text, JSON, or Markdown. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_query_core::{parse_extract, parse_format, query};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default)]
    extract: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    include_line_numbers: bool,
}

/// Single source for the chat schema (and CLI). Keep in lockstep with the
/// authored schema asserted in the drift-guard test below.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("markdown").required().describe("The Markdown document to query."))
        .param(Param::enumv("extract", ["headings", "links", "images", "code_blocks", "tables"]).default("headings").describe("Which elements to pull out: headings (default), links, images, code_blocks, or tables."))
        .param(Param::enumv("format", ["text", "json", "markdown"]).default("text").describe("Output format: text (human-readable, default), json (structured {count, items}), or markdown (reconstructed Markdown for each item)."))
        .param(Param::boolean("include_line_numbers").default(false).describe("Annotate each item with the 1-based source line it starts on. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract headings, links, images, code blocks, or tables from Markdown",
    skill(
        description = "Query a Markdown document like jq for markup. Set extract to headings (default), links, images, code_blocks, or tables to pull out that element. format is text (human-readable, default), json (structured {count, items}), or markdown (reconstructed Markdown). include_line_numbers annotates each item with its 1-based source line. Returns the extracted items in the chosen format.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-query", |a: Args| {
            let extract = parse_extract(&a.extract).map_err(SkillError::InvalidArgs)?;
            let format = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            query(&a.markdown, extract, format, a.include_line_numbers)
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "markdown": { "type": "string", "description": "The Markdown document to query." },
                    "extract":  { "type": "string", "enum": ["headings", "links", "images", "code_blocks", "tables"], "default": "headings", "description": "Which elements to pull out: headings (default), links, images, code_blocks, or tables." },
                    "format":   { "type": "string", "enum": ["text", "json", "markdown"], "default": "text", "description": "Output format: text (human-readable, default), json (structured {count, items}), or markdown (reconstructed Markdown for each item)." },
                    "include_line_numbers": { "type": "boolean", "default": false, "description": "Annotate each item with the 1-based source line it starts on. Default false." }
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
