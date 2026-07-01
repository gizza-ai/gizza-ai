//! gizza-ai/rake-keywords — extract keywords/keyphrases from a document with the
//! RAKE algorithm. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rake_keywords_core::rake;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    top_n: u32,
    #[serde(default)]
    max_words: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The document text to extract keywords from."),
        )
        .param(
            Param::integer("top_n")
                .default(10)
                .min(0.0)
                .describe("Maximum number of keyphrases to return; 0 returns all."),
        )
        .param(
            Param::integer("max_words")
                .default(0)
                .min(0.0)
                .describe("Maximum words per keyphrase; 0 means no limit."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RakeKeywords;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rake-keywords",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract keywords/keyphrases from a document with the RAKE algorithm.",
    skill(
        description = "Extract the most relevant keywords and keyphrases from a document using the RAKE (Rapid Automatic Keyword Extraction) algorithm — no training data needed. Splits the text into candidate phrases at stopwords/punctuation, scores each word by degree/frequency, and ranks phrases by their summed word scores. Set top_n to limit results (0 = all) and max_words to cap phrase length (0 = no cap). Returns the ranked keyphrases with their RAKE scores. Runs locally.",
        parameters = schema_json()
    ),
)]
impl RakeKeywords {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rake-keywords", |a: Args| {
            Ok::<_, SkillError>(rake(&a.text, a.top_n as usize, a.max_words as usize))
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
                    "text": { "type": "string", "description": "The document text to extract keywords from." },
                    "top_n": { "type": "integer", "default": 10, "minimum": 0, "description": "Maximum number of keyphrases to return; 0 returns all." },
                    "max_words": { "type": "integer", "default": 0, "minimum": 0, "description": "Maximum words per keyphrase; 0 means no limit." }
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
