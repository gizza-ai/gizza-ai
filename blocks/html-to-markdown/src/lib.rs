//! gizza-ai/html-to-markdown — convert HTML into clean Markdown.
//!
//! Thin chat-skill wrapper around `gizza-ai-html-to-markdown-core` (htmd). The
//! chat schema is derived from `descriptor()` (single source — chat + CLI + page
//! query-params); the handler delegates to `block_utils::run_skill`. Pure — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_to_markdown_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("html")
            .required()
            .describe("The HTML fragment or page body to convert to Markdown."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert HTML to clean Markdown",
    skill(
        description = "Convert an HTML fragment or page body into clean Markdown, preserving headings, links, lists, code blocks, tables, blockquotes, and emphasis. Pass the HTML as `html`.",
        parameters = schema_json()
    )
)]
impl HtmlToMarkdown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-to-markdown", |a: Args| {
            convert(&a.html).map_err(SkillError::InvalidArgs)
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
                    "html": { "type": "string", "description": "The HTML fragment or page body to convert to Markdown." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
