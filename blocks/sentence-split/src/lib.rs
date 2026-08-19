//! gizza-ai/sentence-split — chat skill block on the shared tool abstraction.
//! Segments plain text into individual sentences with a deterministic
//! rule-based English boundary detector. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill and the pure logic lives in
//! gizza-ai-sentence-split-core. No host calls — runs entirely in the sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "lines".into()
}
fn default_newlines() -> String {
    "paragraph".into()
}
fn default_trim() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_newlines")]
    newlines: String,
    #[serde(default = "default_trim")]
    trim: bool,
    #[serde(default)]
    min_chars: u32,
    #[serde(default)]
    extra_abbreviations: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to split into sentences. Plain text, up to 500000 characters — e.g. 'Dr. Green paid $99.99. It arrived on Mar. 3.'"),
        )
        .param(
            Param::enumv("format", ["lines", "numbered", "blank-line", "json"])
                .default("lines")
                .describe("How to render the sentences. 'lines' (default) = one sentence per line; 'numbered' = one per line prefixed '1. ', '2. '; 'blank-line' = separated by an empty line; 'json' = {\"count\":N,\"sentences\":[{index,text,words,characters}]}."),
        )
        .param(
            Param::enumv("newlines", ["paragraph", "never", "always"])
                .default("paragraph")
                .describe("How line breaks affect boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, only punctuation ends a sentence; 'always' = every line break ends a sentence (use for lists, subtitles, one-per-line text)."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Trim each sentence and fold a line break inside a sentence to a single space. Default true. Set false to preserve spacing inside each sentence while still omitting separator whitespace between sentences."),
        )
        .param(
            Param::integer("min_chars")
                .default(0)
                .min(0.0)
                .max(gizza_ai_sentence_split_core::MAX_MIN_CHARS as f64)
                .describe("Drop sentences shorter than this many characters — useful for clearing stray fragments like 'Yes.'. Default 0 (keep every sentence), maximum 10000."),
        )
        .param(
            Param::string("extra_abbreviations")
                .default("")
                .describe("Extra abbreviations that must never end a sentence, on top of the built-in list (Dr., Mrs., e.g., No., …). Comma-, semicolon- or space-separated, trailing period optional, case-insensitive — e.g. 'Corp., Ltd., Inc.'. Default empty."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sentence-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split text into sentences with abbreviation- and decimal-aware boundaries.",
    skill(
        description = "Split plain text into individual sentences. The rule-based detector keeps abbreviations and titles (Dr., Mrs., e.g., No. 5), initials (J. R. R.), decimals and versions ($99.99, 1.2.3), list markers ('1. Buy milk'), ellipses and quoted speech (\"Stop!\" he said.) from splitting mid-sentence, and also recognises the full-width terminators 。！？. Choose format='lines' (default), 'numbered', 'blank-line' or 'json' (per-sentence index, text, word count and character count plus a total). newlines controls line breaks: 'paragraph' (default, only a blank line breaks), 'never' or 'always'. trim (default true) trims each sentence, min_chars drops short fragments, and extra_abbreviations adds domain abbreviations to the never-split list.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sentence-split", |a: Args| {
            gizza_ai_sentence_split_core::run(
                &a.text,
                &a.format,
                &a.newlines,
                a.trim,
                a.min_chars as usize,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to split into sentences. Plain text, up to 500000 characters — e.g. 'Dr. Green paid $99.99. It arrived on Mar. 3.'" },
                    "format": { "type": "string", "enum": ["lines", "numbered", "blank-line", "json"], "default": "lines", "description": "How to render the sentences. 'lines' (default) = one sentence per line; 'numbered' = one per line prefixed '1. ', '2. '; 'blank-line' = separated by an empty line; 'json' = {\"count\":N,\"sentences\":[{index,text,words,characters}]}." },
                    "newlines": { "type": "string", "enum": ["paragraph", "never", "always"], "default": "paragraph", "description": "How line breaks affect boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, only punctuation ends a sentence; 'always' = every line break ends a sentence (use for lists, subtitles, one-per-line text)." },
                    "trim": { "type": "boolean", "default": true, "description": "Trim each sentence and fold a line break inside a sentence to a single space. Default true. Set false to preserve spacing inside each sentence while still omitting separator whitespace between sentences." },
                    "min_chars": { "type": "integer", "default": 0, "minimum": 0, "maximum": 10000, "description": "Drop sentences shorter than this many characters — useful for clearing stray fragments like 'Yes.'. Default 0 (keep every sentence), maximum 10000." },
                    "extra_abbreviations": { "type": "string", "default": "", "description": "Extra abbreviations that must never end a sentence, on top of the built-in list (Dr., Mrs., e.g., No., …). Comma-, semicolon- or space-separated, trailing period optional, case-insensitive — e.g. 'Corp., Ltd., Inc.'. Default empty." }
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
