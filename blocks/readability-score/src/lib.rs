//! gizza-ai/readability-score — chat skill block. Computes English readability
//! grades (Flesch Reading Ease, Flesch-Kincaid Grade, Gunning-Fog, SMOG) plus
//! the underlying counts. Chat schema single-sourced from descriptor(); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_readability_score_core::analyze;
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
            .describe("The text to score for readability."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/readability-score",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Readability grades (Flesch-Kincaid, Gunning-Fog, SMOG) for text",
    skill(
        description = "Score English text for readability and return the classic indices — Flesch Reading Ease (0–100, higher is easier), Flesch-Kincaid Grade Level, Gunning-Fog Index, SMOG Index, Coleman-Liau Index and the Automated Readability Index — along with the counts they derive from (words, sentences, syllables, complex words) and a plain-language reading level. Runs locally; the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "readability-score", |a: Args| {
            if a.text.trim().is_empty() {
                return Err(SkillError::InvalidArgs("text must not be empty".into()));
            }
            Ok(analyze(&a.text))
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
                    "text": { "type": "string", "description": "The text to score for readability." }
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
