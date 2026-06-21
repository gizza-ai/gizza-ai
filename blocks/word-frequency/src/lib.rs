//! gizza-ai/word-frequency — count how often each word occurs and rank them.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, ToolDescriptor};
use gizza_ai_word_frequency_core::count;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_min_length")]
    min_length: u32,
    #[serde(default)]
    ignore_stopwords: bool,
    #[serde(default)]
    top: u32,
}
fn default_min_length() -> u32 {
    1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose words to count."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When false (default), 'The' and 'the' are grouped; true counts casings separately."),
        )
        .param(
            Param::integer("min_length")
                .min(1.0)
                .max(100.0)
                .describe("Ignore words shorter than this many characters (default 1 = keep all)."),
        )
        .param(
            Param::boolean("ignore_stopwords")
                .default(false)
                .describe("When true, drop common English stop words (the, and, of, to, …)."),
        )
        .param(
            Param::integer("top")
                .min(0.0)
                .describe("Keep only the N most frequent words (0 = all, the default)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WordFrequency;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/word-frequency",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count and rank the most frequent words in text",
    skill(
        description = "Count how often each word occurs in a block of text and rank them from most to least frequent. Returns each distinct word with its count and its percentage of all counted words (keyword density), plus the number of distinct words and total words counted. Words are runs of letters/digits with interior apostrophes kept (don't stays one word); punctuation separates. Options: case_sensitive (default false groups 'The'/'the'), min_length to skip short words, ignore_stopwords to drop common words like 'the'/'and', and top to keep only the N most frequent. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl WordFrequency {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "word-frequency", |a: Args| {
            Ok(count(
                &a.text,
                a.case_sensitive,
                a.min_length as usize,
                a.ignore_stopwords,
                a.top as usize,
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
                    "text": { "type": "string", "description": "The text whose words to count." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When false (default), 'The' and 'the' are grouped; true counts casings separately." },
                    "min_length": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Ignore words shorter than this many characters (default 1 = keep all)." },
                    "ignore_stopwords": { "type": "boolean", "default": false, "description": "When true, drop common English stop words (the, and, of, to, …)." },
                    "top": { "type": "integer", "minimum": 0, "description": "Keep only the N most frequent words (0 = all, the default)." }
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
