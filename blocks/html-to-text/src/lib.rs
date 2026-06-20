//! gizza-ai/html-to-text — strip HTML to clean, readable plain text.
//!
//! Thin chat-skill wrapper around `gizza-ai-html-to-text-core` (nanohtml2text).
//! Chat schema single-sourced from `descriptor()`; handler delegates to
//! `block_utils::run_skill`. Pure — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_to_text_core::convert;
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
            .describe("The HTML fragment or page body to strip to plain text."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlToText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip HTML to clean plain text",
    skill(
        description = "Strip HTML markup to clean, readable plain text, preserving paragraph and list structure (no Markdown markers). Pass the HTML as `html`.",
        parameters = schema_json()
    )
)]
impl HtmlToText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-to-text", |a: Args| {
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
                    "html": { "type": "string", "description": "The HTML fragment or page body to strip to plain text." }
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
