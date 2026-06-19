//! gizza-ai/word-count — counts words, characters, and lines in a block of text.
//!
//! Thin chat-skill wrapper around `gizza-ai-word-count-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`, which wraps the result in
//! `{ "result": "N words, N characters, N lines" }`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("The text to analyze."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WordCount;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/word-count",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Word Count skill",
    skill(
        description = "Count the words, characters, and lines in a block of text.",
        parameters = schema_json()
    )
)]
impl WordCount {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — word-count's
        // existing success shape — and routes errors through GuestResult::error.
        match run_skill(&body, "word-count", |a: Args| {
            gizza_ai_word_count_core::count(&a.text).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// now emits `additionalProperties: false` uniformly, which word-count's
    /// authored schema already had — so this is an exact match.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to analyze." }
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
