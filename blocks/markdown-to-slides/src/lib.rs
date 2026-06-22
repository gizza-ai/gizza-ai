//! gizza-ai/markdown-to-slides — convert Markdown into a self-contained HTML
//! slide deck.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-to-slides-core`
//! (pulldown-cmark + ammonia). The chat schema is single-sourced from
//! `descriptor()` (chat + CLI + page query-params); the handler delegates to
//! `block_utils::run_skill`. Pure — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_to_slides_core::build_slides;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default)]
    title: String,
}

fn default_theme() -> String {
    "light".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown document to turn into slides. Slides are separated by a thematic break (`---`, `***`, or `___`) on its own line."),
        )
        .param(
            Param::enumv("theme", ["light", "dark"])
                .default("light")
                .describe("Color theme for the generated deck."),
        )
        .param(
            Param::string("title")
                .describe("Title for the generated HTML document (the browser tab). Defaults to \"Slides\"."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-slides",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Markdown into a self-contained HTML slide deck",
    skill(
        description = "Convert a Markdown document into a single self-contained, navigable HTML slide deck. Each slide is separated by a thematic break (`---`, `***`, or `___`) on its own line. Markdown is rendered with CommonMark plus GitHub-flavored extensions (tables, task lists, strikethrough, footnotes) and sanitized, then wrapped in one HTML file with embedded CSS and keyboard/click/swipe navigation, a slide counter, and a progress bar — no external dependencies, no build step. Pass the document as `markdown`; optionally set `theme` (\"light\"|\"dark\") and a document `title`.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-to-slides", |a: Args| {
            build_slides(&a.markdown, &a.theme, &a.title).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown document to turn into slides. Slides are separated by a thematic break (`---`, `***`, or `___`) on its own line." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Color theme for the generated deck." },
                    "title": { "type": "string", "description": "Title for the generated HTML document (the browser tab). Defaults to \"Slides\"." }
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
