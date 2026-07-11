//! gizza-ai/spell-check — detect misspelled English words and suggest corrections.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure (bundled dictionary, no model, no network) → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, ToolDescriptor};
use gizza_ai_spell_check_core::check;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_max_suggestions")]
    max_suggestions: u32,
    #[serde(default = "default_true")]
    ignore_uppercase: bool,
    #[serde(default)]
    ignore_capitalized: bool,
    #[serde(default)]
    custom_words: String,
}
fn default_max_suggestions() -> u32 {
    5
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to spell-check (English)."),
        )
        .param(
            Param::integer("max_suggestions")
                .min(1.0)
                .max(20.0)
                .describe("How many correction suggestions to return per misspelled word (default 5)."),
        )
        .param(
            Param::boolean("ignore_uppercase")
                .default(true)
                .describe("When true (default), skip ALL-CAPS tokens like NASA or HTML (treated as acronyms)."),
        )
        .param(
            Param::boolean("ignore_capitalized")
                .default(false)
                .describe("When true, skip Capitalized words (likely names/proper nouns). Default false so sentence-start typos are still caught."),
        )
        .param(
            Param::string("custom_words")
                .describe("Extra words to treat as correctly spelled (names, jargon, brand terms), separated by commas, spaces, or new lines."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpellCheck;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spell-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect misspelled words and suggest corrections",
    skill(
        description = "Spell-check a block of English text and suggest corrections. Returns each misspelled word with its character offset and a ranked list of suggestions (most likely first), the count of misspellings, the number of words checked, and a fully corrected version of the text (each misspelling replaced by its top suggestion, casing preserved). Suggestions come from a bundled ~84k-word English dictionary via Damerau-Levenshtein edit distance ranked by word frequency. Options: max_suggestions (per word, default 5), ignore_uppercase (skip ACRONYMS, default true), ignore_capitalized (skip proper nouns, default false), and custom_words (extra correctly-spelled words). Words with digits or single letters are skipped. It checks spelling only — not grammar, punctuation, or real-word errors (a correctly spelled word used in the wrong place). Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl SpellCheck {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "spell-check", |a: Args| {
            Ok(check(
                &a.text,
                a.max_suggestions as usize,
                a.ignore_uppercase,
                a.ignore_capitalized,
                &a.custom_words,
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
                    "text": { "type": "string", "description": "The text to spell-check (English)." },
                    "max_suggestions": { "type": "integer", "minimum": 1, "maximum": 20, "description": "How many correction suggestions to return per misspelled word (default 5)." },
                    "ignore_uppercase": { "type": "boolean", "default": true, "description": "When true (default), skip ALL-CAPS tokens like NASA or HTML (treated as acronyms)." },
                    "ignore_capitalized": { "type": "boolean", "default": false, "description": "When true, skip Capitalized words (likely names/proper nouns). Default false so sentence-start typos are still caught." },
                    "custom_words": { "type": "string", "description": "Extra words to treat as correctly spelled (names, jargon, brand terms), separated by commas, spaces, or new lines." }
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
