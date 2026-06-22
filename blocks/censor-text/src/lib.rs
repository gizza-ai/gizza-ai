//! gizza-ai/censor-text — redact a list of words (or a built-in list) in text.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_censor_text_core::censor;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    words: String,
    #[serde(default)]
    mask: String,
    #[serde(default = "default_true")]
    whole_word: bool,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text to censor."))
        .param(Param::string("words").default("").describe("Comma-separated words to redact. Leave empty to use a built-in common-profanity list."))
        .param(Param::string("mask").default("*").describe("Masking character (its first char is used). Default '*'."))
        .param(Param::boolean("whole_word").default(true).describe("When true (default), only whole-word matches are masked; false also masks matches inside larger words."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CensorText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/censor-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Redact words in text by masking",
    skill(
        description = "Redact words in text by masking them with a character. Provide a comma-separated `words` list (or leave it empty to use a built-in common-profanity list). Matching is case-insensitive; by default only whole words are masked (set whole_word=false to also catch matches inside larger words). `mask` sets the masking character (default '*').",
        parameters = schema_json()
    )
)]
impl CensorText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "censor-text", |a: Args| {
            censor(&a.text, &a.words, &a.mask, a.whole_word).map_err(SkillError::InvalidArgs)
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
                    "text":       { "type": "string", "description": "The text to censor." },
                    "words":      { "type": "string", "default": "", "description": "Comma-separated words to redact. Leave empty to use a built-in common-profanity list." },
                    "mask":       { "type": "string", "default": "*", "description": "Masking character (its first char is used). Default '*'." },
                    "whole_word": { "type": "boolean", "default": true, "description": "When true (default), only whole-word matches are masked; false also masks matches inside larger words." }
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
