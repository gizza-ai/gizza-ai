//! gizza-ai/extract-urls — extract http(s) URLs from text, dedupe, optionally
//! split into components. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_urls_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    split_components: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan for URLs."),
        )
        .param(
            Param::boolean("split_components")
                .default(false)
                .describe("When true, also split each URL into scheme/host/port/path/query/fragment."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractUrls;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-urls",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract http(s) URLs from text",
    skill(
        description = "Scan text and extract all http/https URLs, validated and deduplicated in first-seen order (trailing prose punctuation is trimmed). Set split_components=true to also break each URL into scheme, host, port, path, query, and fragment. Returns the count, the URL list, and (optionally) the component breakdown. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractUrls {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-urls", |a: Args| {
            Ok::<_, SkillError>(extract(&a.text, a.split_components))
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
                    "text": { "type": "string", "description": "The text to scan for URLs." },
                    "split_components": { "type": "boolean", "default": false, "description": "When true, also split each URL into scheme/host/port/path/query/fragment." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
