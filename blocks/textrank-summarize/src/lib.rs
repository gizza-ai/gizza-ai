//! gizza-ai/textrank-summarize — extractive text summary via TextRank. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_textrank_summarize_core::summarize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_sentences")]
    sentences: u32,
}
fn default_sentences() -> u32 {
    3
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to summarize (a few sentences or more)."),
        )
        .param(
            Param::integer("sentences")
                .min(1.0)
                .max(50.0)
                .describe("How many sentences the summary should contain (default 3)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextrankSummarize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/textrank-summarize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extractive text summary via TextRank",
    skill(
        description = "Summarize text by extracting its most important sentences using the TextRank algorithm (a graph-based PageRank over sentence similarity — no AI model, fully local and deterministic). Returns the top `sentences` sentences (default 3) in their original order, verbatim from the source. Best for articles, reports, or transcripts.",
        parameters = schema_json()
    ),
)]
impl TextrankSummarize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "textrank-summarize", |a: Args| {
            Ok(summarize(&a.text, a.sentences as usize))
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
                    "text": { "type": "string", "description": "The text to summarize (a few sentences or more)." },
                    "sentences": { "type": "integer", "minimum": 1, "maximum": 50, "description": "How many sentences the summary should contain (default 3)." }
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
