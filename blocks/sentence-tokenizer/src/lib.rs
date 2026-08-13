//! gizza-ai/sentence-tokenizer — chat skill block on the shared tool abstraction.
//! Splits plain text into sentences and into word / number / punctuation tokens,
//! each carrying its character span in the source text. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill and the pure logic lives in
//! gizza-ai-sentence-tokenizer-core. No host calls — runs entirely in the sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "json".into()
}
fn default_newlines() -> String {
    "paragraph".into()
}
fn default_split_contractions() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_newlines")]
    newlines: String,
    #[serde(default = "default_split_contractions")]
    split_contractions: bool,
    #[serde(default)]
    split_hyphenated: bool,
    #[serde(default)]
    lowercase: bool,
    #[serde(default)]
    drop_punctuation: bool,
    #[serde(default)]
    extra_abbreviations: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .multiline()
                .describe("The text to tokenize. Plain text, up to 500000 characters — e.g. 'Dr. Green paid $99.99. It works.'. Line breaks are meaningful input: they interact with the newlines parameter."),
        )
        .param(
            Param::enumv("format", ["json", "table", "lines", "spaces", "sentences"])
                .default("json")
                .describe("How to render the token stream. 'json' (default) = {\"counts\":{...},\"sentences\":[{index,start,end,text,tokens:[{index,start,end,type,text}]}]}; 'table' = tab-separated rows (sentence, token, start, end, type, text) with a header line; 'lines' = one token per line; 'spaces' = one sentence per line with tokens separated by single spaces; 'sentences' = one sentence per line of original text."),
        )
        .param(
            Param::enumv("newlines", ["paragraph", "never", "always"])
                .default("paragraph")
                .describe("How line breaks affect sentence boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, only punctuation ends a sentence; 'always' = every line break ends a sentence (use for lists, subtitles, one-per-line text)."),
        )
        .param(
            Param::boolean("split_contractions")
                .default(true)
                .describe("Split contractions Penn Treebank style: don't -> do + n't, Anna's -> Anna + 's, we'll -> we + 'll. Default true. Set false to keep each contraction as one token. Split pieces keep their exact character offsets."),
        )
        .param(
            Param::boolean("split_hyphenated")
                .default(false)
                .describe("Split hyphenated compounds into their parts plus the hyphens: state-of-the-art -> state + - + of + - + the + - + art. Default false, which keeps the compound as a single word token."),
        )
        .param(
            Param::boolean("lowercase")
                .default(false)
                .describe("Lowercase the emitted token and sentence text. Default false. Offsets always point at the original, unmodified text, so a lowercased token can still be mapped back to its source span."),
        )
        .param(
            Param::boolean("drop_punctuation")
                .default(false)
                .describe("Drop punctuation and symbol tokens from the output, leaving words, numbers, URLs and e-mail addresses. Default false. Sentence boundaries are still detected from the punctuation before it is dropped."),
        )
        .param(
            Param::string("extra_abbreviations")
                .default("")
                .describe("Extra abbreviations that must never end a sentence, on top of the built-in list (Dr., Mrs., e.g., No., Inc., …). Comma-, semicolon- or space-separated, trailing period optional, case-insensitive — e.g. 'Blarg., Zyx.'. Default empty."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sentence-tokenizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split text into sentences and tokens with character offsets.",
    skill(
        description = "Tokenize plain text into sentences and into word, number, punctuation, symbol, URL and e-mail tokens, each with its start/end character offset in the original text. The rule-based segmenter keeps abbreviations and titles (Dr., Mrs., e.g., No. 5), initials (J. R. R.), decimals and versions ($99.99, 1.2.3), list markers ('1. Buy milk'), ellipses, quoted speech and full-width terminators 。！？ from splitting mid-sentence, and keeps URLs, e-mail addresses and numbers with internal separators (1,000.00, 2018-11-11) as single tokens. format='json' (default) returns counts plus sentences with per-token spans and types; 'table' is tab-separated rows with offsets; 'lines' is one token per line; 'spaces' re-joins each sentence's tokens with single spaces; 'sentences' is one sentence per line. newlines controls line breaks: 'paragraph' (default), 'never' or 'always'. split_contractions (default true) cuts don't into do + n't, split_hyphenated splits state-of-the-art, lowercase lowercases the emitted text while keeping original offsets, drop_punctuation removes punctuation and symbol tokens, and extra_abbreviations adds domain abbreviations to the never-split list.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sentence-tokenizer", |a: Args| {
            gizza_ai_sentence_tokenizer_core::run(
                &a.text,
                &a.format,
                &a.newlines,
                a.split_contractions,
                a.split_hyphenated,
                a.lowercase,
                a.drop_punctuation,
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
                    "text": {
                        "type": "string",
                        "description": "The text to tokenize. Plain text, up to 500000 characters — e.g. 'Dr. Green paid $99.99. It works.'. Line breaks are meaningful input: they interact with the newlines parameter."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "table", "lines", "spaces", "sentences"],
                        "default": "json",
                        "description": "How to render the token stream. 'json' (default) = {\"counts\":{...},\"sentences\":[{index,start,end,text,tokens:[{index,start,end,type,text}]}]}; 'table' = tab-separated rows (sentence, token, start, end, type, text) with a header line; 'lines' = one token per line; 'spaces' = one sentence per line with tokens separated by single spaces; 'sentences' = one sentence per line of original text."
                    },
                    "newlines": {
                        "type": "string",
                        "enum": ["paragraph", "never", "always"],
                        "default": "paragraph",
                        "description": "How line breaks affect sentence boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, only punctuation ends a sentence; 'always' = every line break ends a sentence (use for lists, subtitles, one-per-line text)."
                    },
                    "split_contractions": {
                        "type": "boolean",
                        "default": true,
                        "description": "Split contractions Penn Treebank style: don't -> do + n't, Anna's -> Anna + 's, we'll -> we + 'll. Default true. Set false to keep each contraction as one token. Split pieces keep their exact character offsets."
                    },
                    "split_hyphenated": {
                        "type": "boolean",
                        "default": false,
                        "description": "Split hyphenated compounds into their parts plus the hyphens: state-of-the-art -> state + - + of + - + the + - + art. Default false, which keeps the compound as a single word token."
                    },
                    "lowercase": {
                        "type": "boolean",
                        "default": false,
                        "description": "Lowercase the emitted token and sentence text. Default false. Offsets always point at the original, unmodified text, so a lowercased token can still be mapped back to its source span."
                    },
                    "drop_punctuation": {
                        "type": "boolean",
                        "default": false,
                        "description": "Drop punctuation and symbol tokens from the output, leaving words, numbers, URLs and e-mail addresses. Default false. Sentence boundaries are still detected from the punctuation before it is dropped."
                    },
                    "extra_abbreviations": {
                        "type": "string",
                        "default": "",
                        "description": "Extra abbreviations that must never end a sentence, on top of the built-in list (Dr., Mrs., e.g., No., Inc., …). Comma-, semicolon- or space-separated, trailing period optional, case-insensitive — e.g. 'Blarg., Zyx.'. Default empty."
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
