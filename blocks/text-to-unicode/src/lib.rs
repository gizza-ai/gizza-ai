//! gizza-ai/text-to-unicode — list each character's Unicode code point, name and
//! escape sequence for the given text. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_to_unicode_core::{to_unicode, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_format")]
    format: String,
}
fn default_format() -> String {
    "table".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text to break down into Unicode code points."))
        .param(
            Param::enumv("format", ["table", "json"]).default("table").describe(
                "Output format: table (default, aligned ASCII grid with Char/Code Point/Dec/Escape/UTF-8/Name columns) or json (an array of objects).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextToUnicode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-to-unicode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List each character's Unicode code point, name and escape sequence",
    skill(
        description = "Break text into its Unicode scalar values and, for each character, report the code point (U+XXXX), decimal value, official Unicode name, the \\u escape sequence (\\uXXXX for the BMP, \\u{XXXX} for astral code points like emoji), and the UTF-8 bytes in hex. format=table (default) returns an aligned ASCII grid; format=json returns an array of objects. Useful for spotting invisible/look-alike characters, debugging encoding issues, and writing escape sequences. Runs locally.",
        parameters = schema_json()
    ),
)]
impl TextToUnicode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-to-unicode", |a: Args| {
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            to_unicode(&a.text, fmt).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "The text to break down into Unicode code points." },
                    "format": { "type": "string", "enum": ["table", "json"], "default": "table", "description": "Output format: table (default, aligned ASCII grid with Char/Code Point/Dec/Escape/UTF-8/Name columns) or json (an array of objects)." }
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
