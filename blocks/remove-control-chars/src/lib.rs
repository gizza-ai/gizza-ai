//! gizza-ai/remove-control-chars — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, ToolDescriptor};
use gizza_ai_remove_control_chars_core::remove_control_chars;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    keep_tabs: bool,
    #[serde(default = "default_true")]
    keep_newlines: bool,
    #[serde(default)]
    replacement: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to strip control characters from."),
        )
        .param(
            Param::boolean("keep_tabs").default(true).describe(
                "When true (default), keep horizontal tab characters (\\t). Set false to remove them too.",
            ),
        )
        .param(
            Param::boolean("keep_newlines").default(true).describe(
                "When true (default), keep line feed (\\n) and carriage return (\\r) characters. Set false to remove them too.",
            ),
        )
        .param(
            Param::string("replacement").default("").describe(
                "String to substitute for each removed control character. Defaults to an empty string, which deletes the character.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/remove-control-chars",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip null bytes and non-printable control characters from text",
    skill(
        description = "Remove null bytes and non-printable control characters (Unicode category Cc: U+0000–U+001F, U+007F–U+009F) from text. By default tabs (keep_tabs) and newlines/carriage returns (keep_newlines) are preserved as meaningful whitespace; set them false to strip those too. replacement is substituted for each removed character (empty string by default, which deletes it). Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "remove-control-chars", |a: Args| {
            Ok(remove_control_chars(
                &a.text,
                a.keep_tabs,
                a.keep_newlines,
                &a.replacement,
            ))
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
                    "text": { "type": "string", "description": "The text to strip control characters from." },
                    "keep_tabs": { "type": "boolean", "default": true, "description": "When true (default), keep horizontal tab characters (\\t). Set false to remove them too." },
                    "keep_newlines": { "type": "boolean", "default": true, "description": "When true (default), keep line feed (\\n) and carriage return (\\r) characters. Set false to remove them too." },
                    "replacement": { "type": "string", "default": "", "description": "String to substitute for each removed control character. Defaults to an empty string, which deletes the character." }
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
