//! gizza-ai/tweet-thread-splitter — splits long text into numbered,
//! character-limit-safe tweet chunks that never break a word mid-word.
//!
//! Thin chat-skill wrapper around `gizza-ai-tweet-thread-splitter-core`. The
//! chat schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_tweet_thread_splitter_core::{split, MAX_LIMIT, MIN_LIMIT};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// Max characters per tweet (0 → 280). Core clamps to MIN_LIMIT..=MAX_LIMIT.
    #[serde(default)]
    limit: usize,
    /// Thread-counter style: "parens" (default) | "slash" | "dotted" | "none".
    #[serde(default)]
    numbering: String,
    /// Length counting: "chars" (default) or "utf16".
    #[serde(default)]
    count: String,
    /// Prefer breaking on sentence boundaries (default true).
    #[serde(default = "default_true")]
    prefer_sentences: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The long text to split into a tweet thread."),
        )
        .param(
            Param::integer("limit")
                .default(280)
                .min(MIN_LIMIT as f64)
                .max(MAX_LIMIT as f64)
                .describe("Maximum characters per tweet, 10-25000. Default 280 (X/Twitter's standard limit). The numbering counter counts toward this, so each tweet stays at or under the limit."),
        )
        .param(
            Param::enumv("numbering", ["parens", "slash", "dotted", "none"])
                .default("parens")
                .describe("Thread-counter style. 'parens' (default) appends ' (i/N)'; 'slash' appends ' i/N'; 'dotted' prepends 'i. ' (numbered-list style); 'none' adds no counter."),
        )
        .param(
            Param::enumv("count", ["chars", "utf16"])
                .default("chars")
                .describe("How tweet length is measured. 'chars' (default) counts Unicode characters; 'utf16' counts UTF-16 code units, matching how X and most JavaScript clients weigh emoji and astral characters (each as 2)."),
        )
        .param(
            Param::boolean("prefer_sentences")
                .default(true)
                .describe("When true (default), start a new tweet on a sentence boundary ('. ! ?') where possible so a tweet rarely ends mid-thought; a sentence longer than the limit still falls back to word-packing. A word is never broken."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Split `args.text` and render the thread as plain text (tweets separated by a
/// blank line). Shared by the chat handler.
fn run_thread(a: &Args) -> Result<String, String> {
    let thread = split(&a.text, a.limit, &a.numbering, &a.count, a.prefer_sentences)?;
    Ok(thread.to_plain())
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tweet-thread-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split long text into numbered, character-limit-safe tweet chunks.",
    skill(
        description = "Split a long piece of text into a numbered Twitter/X thread of character-limit-safe tweets that never break a word in half. Set limit to the per-tweet character cap (default 280; the counter counts toward it). numbering chooses the thread-counter style: 'parens' (default) ' (i/N)', 'slash' ' i/N', 'dotted' 'i. ' prefix, or 'none'. count='chars' (default) measures length in Unicode characters; count='utf16' measures UTF-16 code units (matching how X and most JS clients weigh emoji). prefer_sentences=true (default) starts a new tweet on sentence boundaries so tweets rarely end mid-thought. Words longer than a whole tweet (e.g. a long URL) are hard-split so nothing is lost. Returns the tweets separated by blank lines.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tweet-thread-splitter", |a: Args| {
            run_thread(&a).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The long text to split into a tweet thread." },
                    "limit": { "type": "integer", "minimum": 10, "maximum": 25000, "default": 280, "description": "Maximum characters per tweet, 10-25000. Default 280 (X/Twitter's standard limit). The numbering counter counts toward this, so each tweet stays at or under the limit." },
                    "numbering": { "type": "string", "enum": ["parens", "slash", "dotted", "none"], "default": "parens", "description": "Thread-counter style. 'parens' (default) appends ' (i/N)'; 'slash' appends ' i/N'; 'dotted' prepends 'i. ' (numbered-list style); 'none' adds no counter." },
                    "count": { "type": "string", "enum": ["chars", "utf16"], "default": "chars", "description": "How tweet length is measured. 'chars' (default) counts Unicode characters; 'utf16' counts UTF-16 code units, matching how X and most JavaScript clients weigh emoji and astral characters (each as 2)." },
                    "prefer_sentences": { "type": "boolean", "default": true, "description": "When true (default), start a new tweet on a sentence boundary ('. ! ?') where possible so a tweet rarely ends mid-thought; a sentence longer than the limit still falls back to word-packing. A word is never broken." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_thread_numbers_a_multi_tweet_thread() {
        let a = Args {
            text: "aaaaa bbbbb ccccc ddddd eeeee fffff".to_string(),
            limit: 20,
            numbering: "parens".to_string(),
            count: String::new(),
            prefer_sentences: false,
        };
        let out = run_thread(&a).unwrap();
        assert!(out.contains("(1/"));
        assert!(out.contains("\n\n"));
    }
}
