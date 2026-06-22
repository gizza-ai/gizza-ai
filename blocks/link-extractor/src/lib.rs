//! gizza-ai/link-extractor — extract every hyperlink and in-page anchor (jump
//! target) from HTML or Markdown. Pure → all backends. Chat schema is
//! single-sourced from descriptor(); handle() delegates to run_skill, returning
//! the structured Report ({ source, link_count, anchor_count, links, anchors }).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_link_extractor_core::{extract_with, Options, Source};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    dedup: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The HTML or Markdown to scan for links and anchors."),
        )
        .param(
            Param::enumv("source", ["auto", "html", "markdown"])
                .default("auto")
                .describe("How to parse the input: \"auto\" (default, sniffed from the content), \"html\", or \"markdown\"."),
        )
        .param(
            Param::string("base_url")
                .describe("Optional absolute base URL (e.g. \"https://example.com/docs/\"). When set, relative links are resolved to absolute URLs."),
        )
        .param(
            Param::boolean("dedup")
                .default(false)
                .describe("When true, collapse repeated links that share the same final URL (first-seen kept)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LinkExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/link-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract hyperlinks and anchors from HTML or Markdown",
    skill(
        description = "Extract every hyperlink and in-page anchor (jump target) from HTML or Markdown. Set source='auto' (default; sniffed from the content), 'html', or 'markdown'. Pass base_url to resolve relative links to absolute, and dedup=true to collapse repeated links. Returns the parser used plus a list of links (each with its url, visible text, a relative flag, and any HTML rel attribute) and a list of anchors (id/name attributes or heading slugs that a same-document '#name' link would jump to). Runs locally.",
        parameters = schema_json()
    ),
)]
impl LinkExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "link-extractor", |a: Args| {
            let src = Source::parse(&a.source).map_err(SkillError::InvalidArgs)?;
            let opts = Options { base_url: a.base_url, dedup: a.dedup };
            extract_with(&a.input, src, &opts).map_err(SkillError::InvalidArgs)
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
                    "input":    { "type": "string", "description": "The HTML or Markdown to scan for links and anchors." },
                    "source":   { "type": "string", "enum": ["auto", "html", "markdown"], "default": "auto", "description": "How to parse the input: \"auto\" (default, sniffed from the content), \"html\", or \"markdown\"." },
                    "base_url": { "type": "string", "description": "Optional absolute base URL (e.g. \"https://example.com/docs/\"). When set, relative links are resolved to absolute URLs." },
                    "dedup":    { "type": "boolean", "default": false, "description": "When true, collapse repeated links that share the same final URL (first-seen kept)." }
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
