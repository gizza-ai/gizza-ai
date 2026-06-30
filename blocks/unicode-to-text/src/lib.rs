//! gizza-ai/unicode-to-text — decode Unicode escape sequences back into text.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_unicode_to_text_core::decode;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("Text containing Unicode escape sequences to decode back into characters."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UnicodeToText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/unicode-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode Unicode escape sequences back into readable text",
    skill(
        description = "Decode text that contains Unicode escape sequences back into the actual characters. Auto-detects and decodes, all in one pass (no target to pick): \\uXXXX (4-hex JS/JSON/Java, surrogate pairs are combined), \\u{X..} (braced Rust/ES6), \\xXX (2-hex byte), \\U00XXXXXX (8-hex Python), U+XXXX / u+xxxx (code-point notation), and HTML numeric character references &#DDDD; (decimal) and &#xHHHH; (hex). Text that isn't an escape is passed through unchanged; invalid code points become U+FFFD. The inverse of an escaping tool. Runs locally.",
        parameters = schema_json()
    ),
)]
impl UnicodeToText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "unicode-to-text", |a: Args| {
            decode(&a.text).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "Text containing Unicode escape sequences to decode back into characters." }
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
