//! gizza-ai/morse-code — converts text to International Morse code and back.
//!
//! Thin chat-skill wrapper around `gizza-ai-morse-code-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    direction: String,
    /// Separator between letters/symbols (blank → a single space).
    #[serde(default)]
    letter_sep: String,
    /// Separator between words (blank → " / ").
    #[serde(default)]
    word_sep: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to convert to Morse code, or the Morse code to convert back to text."),
        )
        .param(
            Param::enumv("direction", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns plain text into Morse code; 'decode' turns Morse code back into text."),
        )
        .param(
            Param::string("letter_sep")
                .default(" ")
                .describe("Separator placed between the Morse symbols for each letter. Blank uses a single space."),
        )
        .param(
            Param::string("word_sep")
                .default(" / ")
                .describe("Separator placed between words. Blank uses ' / ' (space-slash-space), the common convention."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MorseCode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/morse-code",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Morse code converter skill",
    skill(
        description = "Convert text to International Morse code and Morse code back to text. Use direction='encode' (default) to turn plain text into Morse (e.g. 'SOS' -> '... --- ...'), or direction='decode' to turn Morse back into text. Supports the standard International Morse alphabet: A-Z, 0-9, and common punctuation (. , ? ' ! / ( ) & : ; = + - _ \" $ @). Encoding is case-insensitive; unknown characters become the code for '?'. Letters are separated by letter_sep (default a single space) and words by word_sep (default ' / '); set them to match any convention.",
        parameters = schema_json()
    ),
)]
impl MorseCode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "morse-code", |a: Args| {
            gizza_ai_morse_code_core::convert(&a.text, &a.direction, &a.letter_sep, &a.word_sep)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to convert to Morse code, or the Morse code to convert back to text." },
                    "direction": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns plain text into Morse code; 'decode' turns Morse code back into text." },
                    "letter_sep": { "type": "string", "default": " ", "description": "Separator placed between the Morse symbols for each letter. Blank uses a single space." },
                    "word_sep": { "type": "string", "default": " / ", "description": "Separator placed between words. Blank uses ' / ' (space-slash-space), the common convention." }
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
