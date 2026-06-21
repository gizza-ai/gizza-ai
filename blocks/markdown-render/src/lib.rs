//! gizza-ai/markdown-render — render Markdown into sanitized HTML.
//!
//! Thin chat-skill wrapper around `gizza-ai-markdown-render-core` (pulldown-cmark
//! + ammonia). The chat schema is derived from `descriptor()` (single source —
//! chat + CLI + page query-params); the handler delegates to
//! `block_utils::run_skill`. Pure — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_render_core::render;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("markdown")
            .required()
            .describe("The Markdown text to render into sanitized HTML."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-render",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render Markdown into sanitized HTML",
    skill(
        description = "Render Markdown (CommonMark plus GitHub-flavored extensions: tables, fenced code blocks, task lists, strikethrough, and footnotes) into HTML. The output is sanitized so it is safe to embed (scripts, event handlers, and dangerous URL schemes are stripped). Pass the Markdown as `markdown`.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-render", |a: Args| {
            render(&a.markdown).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown text to render into sanitized HTML." }
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
