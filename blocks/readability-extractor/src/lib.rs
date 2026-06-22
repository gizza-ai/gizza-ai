//! gizza-ai/readability-extractor — pull the main article out of cluttered HTML.
//!
//! Thin chat-skill wrapper around `gizza-ai-readability-extractor-core`
//! (dom_smoothie). Chat schema single-sourced from `descriptor()`; handler
//! delegates to `block_utils::run_skill`. Pure — runs in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_readability_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The cluttered HTML (article page source) to extract the main content from."))
        .param(Param::enumv("format", ["text", "html"]).default("text").describe("Output the cleaned article as plain 'text' (default) or cleaned 'html'."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ReadabilityExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/readability-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the main article from cluttered HTML",
    skill(
        description = "Extract the main article content (title + body) from cluttered HTML, stripping navigation, ads, and boilerplate (a Readability-style extraction). Pass the page HTML as `html`; set format='text' (default) for readable plain text or 'html' for cleaned article HTML.",
        parameters = schema_json()
    )
)]
impl ReadabilityExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "readability-extractor", |a: Args| {
            let as_html = a.format.trim().eq_ignore_ascii_case("html");
            extract(&a.html, as_html).map_err(SkillError::InvalidArgs)
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
                    "html":   { "type": "string", "description": "The cluttered HTML (article page source) to extract the main content from." },
                    "format": { "type": "string", "enum": ["text", "html"], "default": "text", "description": "Output the cleaned article as plain 'text' (default) or cleaned 'html'." }
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
