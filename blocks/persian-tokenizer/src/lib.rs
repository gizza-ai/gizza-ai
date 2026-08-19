//! gizza-ai/persian-tokenizer — chat skill block on the shared tool abstraction.
//! Splits Persian/Farsi text into sentences and words, keeping ZWNJ-joined
//! compounds intact and treating Persian punctuation and digits correctly. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill and the pure logic lives in
//! gizza-ai-persian-tokenizer-core. No host calls — runs entirely in the sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "words".into()
}
fn default_format() -> String {
    "lines".into()
}
fn default_punctuation() -> String {
    "separate".into()
}
fn default_newlines() -> String {
    "paragraph".into()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_punctuation")]
    punctuation: String,
    #[serde(default)]
    split_zwnj: bool,
    #[serde(default = "default_true")]
    normalize: bool,
    #[serde(default = "default_true")]
    keep_entities: bool,
    #[serde(default = "default_newlines")]
    newlines: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The Persian/Farsi text to tokenize. Plain text, up to 200000 characters — e.g. 'ما کتاب می‌خوانیم. یادگیری خوب است.'"),
        )
        .param(
            Param::enumv("mode", ["words", "sentences", "both"])
                .default("words")
                .describe("What to return. 'words' (default) = word tokens for the whole text; 'sentences' = sentence segments only; 'both' = each sentence followed by its own word tokens."),
        )
        .param(
            Param::enumv("format", ["lines", "numbered", "space-separated", "json"])
                .default("lines")
                .describe("How to render the result. 'lines' (default) = one item per line; 'numbered' = one per line prefixed '1. ', '2. '; 'space-separated' = items joined by a single space (the classic tokenizer output 'ما کتاب می‌خوانیم'); 'json' = {\"mode\",\"sentence_count\",\"token_count\",\"tokens\"|\"sentences\"}."),
        )
        .param(
            Param::enumv("punctuation", ["separate", "attach", "remove"])
                .default("separate")
                .describe("What happens to punctuation marks such as ، ؛ ؟ « » . ! ?. 'separate' (default) = each mark is its own token, repeats of one mark grouped ('؟؟', '...'); 'attach' = marks stay glued to the word they touch (split on whitespace only); 'remove' = punctuation tokens are dropped."),
        )
        .param(
            Param::boolean("split_zwnj")
                .default(false)
                .describe("Break ZWNJ half-space compounds (نیم‌فاصله, U+200C) into their parts: 'می‌خوانیم' becomes 'می' + 'خوانیم' and 'کتاب‌ها' becomes 'کتاب' + 'ها'. Default false, which keeps each compound as ONE word — the usual choice for word counts and search indexing."),
        )
        .param(
            Param::boolean("normalize")
                .default(true)
                .describe("Fold Arabic letter forms to Persian (ي→ی, ك→ک, ى→ی, ة→ه), convert Arabic-Indic digits ٠-٩ to Persian ۰-۹, and strip harakat (تشکیل) plus the kashida ـ before tokenizing, so the same word typed on an Arabic keyboard yields the same token. Default true; set false to keep the original characters."),
        )
        .param(
            Param::boolean("keep_entities")
                .default(true)
                .describe("Keep URLs, email addresses, @mentions, #hashtags and separator-bearing numbers whole: 'https://example.com/a', 'info@example.com', '۱۳۹۶/۰۶/۱۱', '1,250.75' each stay one token. Default true; set false to split them at every separator."),
        )
        .param(
            Param::enumv("newlines", ["paragraph", "never", "always"])
                .default("paragraph")
                .describe("How line breaks affect sentence boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, so wrapped lines rejoin; 'always' = every line break ends a sentence (lists, subtitles, one item per line)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/persian-tokenizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split Persian text into sentences and words with correct ZWNJ and punctuation handling.",
    skill(
        description = "Tokenize Persian/Farsi text into words and sentences with a deterministic rule-based segmenter. ZWNJ half-space compounds (می‌خوانیم, کتاب‌ها) stay one word by default and split into their parts with split_zwnj=true. Persian punctuation (، ؛ ؟ « » ۔) is recognised alongside the ASCII marks, and ؟ ۔ ⸮ end a sentence just like ? and .; a period between digits or inside a URL/email never does. All three digit sets (0-9, ٠-٩, ۰-۹) count as digits so ۱۳۹۶/۰۶/۱۱ and 1,250.75 stay whole, as do URLs, emails, @mentions and #hashtags (keep_entities, default true). normalize (default true) folds Arabic ي/ك/ى/ة to Persian and strips harakat and the kashida first. Choose mode='words' (default), 'sentences' or 'both'; format='lines' (default), 'numbered', 'space-separated' or 'json' with sentence and token counts; punctuation='separate' (default), 'attach' or 'remove'; newlines='paragraph' (default), 'never' or 'always'.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "persian-tokenizer", |a: Args| {
            gizza_ai_persian_tokenizer_core::run(
                &a.text,
                &a.mode,
                &a.format,
                &a.punctuation,
                a.split_zwnj,
                a.normalize,
                a.keep_entities,
                &a.newlines,
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
                    "text": { "type": "string", "description": "The Persian/Farsi text to tokenize. Plain text, up to 200000 characters — e.g. 'ما کتاب می‌خوانیم. یادگیری خوب است.'" },
                    "mode": { "type": "string", "enum": ["words", "sentences", "both"], "default": "words", "description": "What to return. 'words' (default) = word tokens for the whole text; 'sentences' = sentence segments only; 'both' = each sentence followed by its own word tokens." },
                    "format": { "type": "string", "enum": ["lines", "numbered", "space-separated", "json"], "default": "lines", "description": "How to render the result. 'lines' (default) = one item per line; 'numbered' = one per line prefixed '1. ', '2. '; 'space-separated' = items joined by a single space (the classic tokenizer output 'ما کتاب می‌خوانیم'); 'json' = {\"mode\",\"sentence_count\",\"token_count\",\"tokens\"|\"sentences\"}." },
                    "punctuation": { "type": "string", "enum": ["separate", "attach", "remove"], "default": "separate", "description": "What happens to punctuation marks such as ، ؛ ؟ « » . ! ?. 'separate' (default) = each mark is its own token, repeats of one mark grouped ('؟؟', '...'); 'attach' = marks stay glued to the word they touch (split on whitespace only); 'remove' = punctuation tokens are dropped." },
                    "split_zwnj": { "type": "boolean", "default": false, "description": "Break ZWNJ half-space compounds (نیم‌فاصله, U+200C) into their parts: 'می‌خوانیم' becomes 'می' + 'خوانیم' and 'کتاب‌ها' becomes 'کتاب' + 'ها'. Default false, which keeps each compound as ONE word — the usual choice for word counts and search indexing." },
                    "normalize": { "type": "boolean", "default": true, "description": "Fold Arabic letter forms to Persian (ي→ی, ك→ک, ى→ی, ة→ه), convert Arabic-Indic digits ٠-٩ to Persian ۰-۹, and strip harakat (تشکیل) plus the kashida ـ before tokenizing, so the same word typed on an Arabic keyboard yields the same token. Default true; set false to keep the original characters." },
                    "keep_entities": { "type": "boolean", "default": true, "description": "Keep URLs, email addresses, @mentions, #hashtags and separator-bearing numbers whole: 'https://example.com/a', 'info@example.com', '۱۳۹۶/۰۶/۱۱', '1,250.75' each stay one token. Default true; set false to split them at every separator." },
                    "newlines": { "type": "string", "enum": ["paragraph", "never", "always"], "default": "paragraph", "description": "How line breaks affect sentence boundaries. 'paragraph' (default) = only a blank line ends a sentence; 'never' = line breaks are ordinary whitespace, so wrapped lines rejoin; 'always' = every line break ends a sentence (lists, subtitles, one item per line)." }
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
