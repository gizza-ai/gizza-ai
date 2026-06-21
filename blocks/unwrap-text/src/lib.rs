//! gizza-ai/unwrap-text — remove hard line breaks inside paragraphs to rejoin
//! wrapped text. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_unwrap_text_core::unwrap;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    keep_list_breaks: bool,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose hard-wrapped lines should be rejoined."),
        )
        .param(
            Param::boolean("keep_list_breaks").default(true).describe(
                "When true (default), keep list items and quote lines (-, *, +, >, '1.') on their own lines; set false to join everything within a paragraph.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UnwrapText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/unwrap-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rejoin hard-wrapped lines into continuous text",
    skill(
        description = "Remove hard line breaks inside paragraphs to rejoin wrapped text into continuous lines, while keeping paragraph breaks (blank lines). Useful for un-wrapping text copied from emails, PDFs, or fixed-width sources. keep_list_breaks (default true) keeps list items and quote lines on their own lines. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl UnwrapText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "unwrap-text", |a: Args| {
            Ok(unwrap(&a.text, a.keep_list_breaks))
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
                    "text": { "type": "string", "description": "The text whose hard-wrapped lines should be rejoined." },
                    "keep_list_breaks": { "type": "boolean", "default": true, "description": "When true (default), keep list items and quote lines (-, *, +, >, '1.') on their own lines; set false to join everything within a paragraph." }
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
