//! gizza-ai/word-count — counts words, characters, and lines in a block of text.
//!
//! Thin chat-skill wrapper around `gizza-ai-word-count-core`. Takes
//! `{ "text": "..." }`, returns `{ "result": "N words, N characters, N lines" }`
//! or `{ "error": "..." }`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
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
        parameters = r#"{
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to analyze." }
            },
            "required": ["text"],
            "additionalProperties": false
        }"#
    ),
)]
impl WordCount {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match gizza_ai_word_count_core::count(&args.text) {
            Ok(v) => GuestResult::respond(
                serde_json::to_vec(&serde_json::json!({ "result": v })).unwrap_or_default(),
            ),
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    GuestResult::respond(
        serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default(),
    )
}
