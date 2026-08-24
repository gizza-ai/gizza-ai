//! gizza-ai/sentence-length-stats — sentence count, average/median/max sentence
//! length, spread, and the distribution of lengths for a block of text. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sentence_length_stats_core::analyze;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_newlines")]
    newlines: String,
    #[serde(default = "default_long_threshold")]
    long_threshold: u32,
    #[serde(default = "default_list_longest")]
    list_longest: u32,
    #[serde(default)]
    extra_abbreviations: String,
}
fn default_newlines() -> String {
    "paragraph".to_string()
}
fn default_long_threshold() -> u32 {
    25
}
fn default_list_longest() -> u32 {
    3
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose sentence lengths to measure. Plain prose; up to 500000 characters."),
        )
        .param(
            Param::enumv("newlines", ["paragraph", "never", "always"])
                .default("paragraph")
                .describe("How line breaks end a sentence: \"paragraph\" (default) breaks only on a blank line, \"never\" treats every line break as ordinary whitespace, \"always\" ends a sentence at every line break (use for subtitles, bullet lists, one-line-per-row text)."),
        )
        .param(
            Param::integer("long_threshold")
                .default(25)
                .min(1.0)
                .max(500.0)
                .describe("Word count at or above which a sentence is reported as long, e.g. 25 (default) for web copy, 20 for plain-language writing."),
        )
        .param(
            Param::integer("list_longest")
                .default(3)
                .min(0.0)
                .max(50.0)
                .describe("How many of the longest sentences to list, longest first, with their position in the text. 0 lists none. Default 3."),
        )
        .param(
            Param::string("extra_abbreviations")
                .default("")
                .describe("Comma-separated extra abbreviations that must never end a sentence, without the trailing period, e.g. \"approx, ing, dept\". Common titles and Latin abbreviations (Dr., e.g., i.e.) are already handled."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SentenceLengthStats;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sentence-length-stats",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sentence count, average/median/max length and length distribution",
    skill(
        description = "Measure how long the sentences in a block of text are. Returns the sentence count, total words and characters, average words and characters per sentence, median length, the shortest and longest sentence (with their positions), the standard deviation, a 0-100 variety score with a plain-language band, how many sentences reach a configurable long-sentence threshold, how often adjacent sentences have near-identical lengths, the distribution across five bands (very short 1-9, short 10-14, medium 15-24, long 25-34, very long 35+), and the N longest sentences. Sentence boundaries use a rule-based detector that keeps abbreviations (Dr., e.g.), initials, decimals and list enumerators intact. Options: newlines (paragraph/never/always), long_threshold, list_longest, extra_abbreviations. Deterministic, no AI model, runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl SentenceLengthStats {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sentence-length-stats", |a: Args| {
            analyze(
                &a.text,
                &a.newlines,
                a.long_threshold as usize,
                a.list_longest as usize,
                &a.extra_abbreviations,
            )
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text whose sentence lengths to measure. Plain prose; up to 500000 characters."
                    },
                    "newlines": {
                        "type": "string",
                        "enum": ["paragraph", "never", "always"],
                        "default": "paragraph",
                        "description": "How line breaks end a sentence: \"paragraph\" (default) breaks only on a blank line, \"never\" treats every line break as ordinary whitespace, \"always\" ends a sentence at every line break (use for subtitles, bullet lists, one-line-per-row text)."
                    },
                    "long_threshold": {
                        "type": "integer",
                        "default": 25,
                        "minimum": 1,
                        "maximum": 500,
                        "description": "Word count at or above which a sentence is reported as long, e.g. 25 (default) for web copy, 20 for plain-language writing."
                    },
                    "list_longest": {
                        "type": "integer",
                        "default": 3,
                        "minimum": 0,
                        "maximum": 50,
                        "description": "How many of the longest sentences to list, longest first, with their position in the text. 0 lists none. Default 3."
                    },
                    "extra_abbreviations": {
                        "type": "string",
                        "default": "",
                        "description": "Comma-separated extra abbreviations that must never end a sentence, without the trailing period, e.g. \"approx, ing, dept\". Common titles and Latin abbreviations (Dr., e.g., i.e.) are already handled."
                    }
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
