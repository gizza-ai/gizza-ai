//! gizza-ai/text-statistics — count words, characters, sentences, paragraphs and
//! lines in text, plus reading/speaking time and averages. Thin wrapper; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_statistics_core::analyze;
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
            .describe("The text to analyze."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextStatistics;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-statistics",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Word/character/sentence stats + reading time for text",
    skill(
        description = "Analyze a block of text and return its statistics: characters (with and without spaces), words, sentences, paragraphs, lines, estimated reading time (200 wpm) and speaking time (130 wpm) in minutes, average word length, and average words per sentence. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl TextStatistics {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-statistics", |a: Args| Ok(analyze(&a.text))) {
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
