//! gizza-ai/text-corpus-cleaner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all
//! backends. The core does the line-oriented cleaning; this file only maps the
//! LLM/CLI args onto core::run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_corpus_cleaner_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_form")]
    unicode_form: String,
    #[serde(default = "default_whitespace")]
    whitespace: String,
    #[serde(default)]
    lowercase: bool,
    #[serde(default = "default_dedupe")]
    dedupe: String,
    #[serde(default = "default_any")]
    language: String,
    #[serde(default)]
    min_chars: u32,
    #[serde(default)]
    min_words: u32,
    #[serde(default = "default_ratio")]
    max_symbol_ratio: f64,
    #[serde(default = "default_true")]
    collapse_blank: bool,
}
fn default_form() -> String { "nfc".to_string() }
fn default_whitespace() -> String { "trim".to_string() }
fn default_dedupe() -> String { "exact".to_string() }
fn default_any() -> String { "any".to_string() }
fn default_ratio() -> f64 { 1.0 }
fn default_true() -> bool { true }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The raw text corpus to clean, one record/sentence per line (e.g. scraped lines or a word list). Lines are split on \\n, \\r\\n, or \\r."))
        .param(Param::enumv("unicode_form", ["none", "nfc", "nfkc"]).default("nfc").describe("Unicode normalization applied to each line before other transforms: 'none' leaves code points untouched; 'nfc' is canonical composition (recommended default); 'nfkc' also folds ligatures (ﬁ→fi), full-width and styled letters, and fractions to plain forms. Default 'nfc'."))
        .param(Param::enumv("whitespace", ["keep", "trim", "collapse"]).default("trim").describe("Per-line whitespace handling: 'keep' leaves it, 'trim' strips only the ends, 'collapse' also squeezes interior runs to one space. Default 'trim'."))
        .param(Param::boolean("lowercase").default(false).describe("Lowercase every line after normalization. Default false."))
        .param(Param::enumv("dedupe", ["none", "exact", "normalized"]).default("exact").describe("Drop duplicate non-blank lines, keeping the first: 'none' keeps all; 'exact' drops byte-identical repeats; 'normalized' also folds case and interior spacing before comparing. Blank lines are never deduplicated. Default 'exact'."))
        .param(Param::enumv("language", ["eng", "spa", "fra", "deu", "ita", "por", "nld", "rus", "ara", "cmn", "jpn", "kor", "hin", "any"]).default("any").describe("Keep only lines detected as this ISO 639-3 language (eng, spa, fra, deu, ita, por, nld, rus, ara, cmn=Chinese, jpn, kor, hin); 'any' disables the filter. Detection is trigram/script based, no model files; lines too short to detect are kept. Default 'any'."))
        .param(Param::integer("min_chars").default(0).min(0.0).describe("Drop lines with fewer than this many characters (counted after transforms). 0 keeps all lengths. Default 0."))
        .param(Param::integer("min_words").default(0).min(0.0).describe("Drop lines with fewer than this many whitespace-separated words. 0 keeps all. Default 0."))
        .param(Param::number("max_symbol_ratio").default(1.0).min(0.0).max(1.0).describe("Drop a line if the ratio of symbol characters (non-alphanumeric, non-space) to visible characters exceeds this (0.0–1.0). 1.0 keeps every line; e.g. 0.5 drops lines that are mostly punctuation. Default 1.0."))
        .param(Param::boolean("collapse_blank").default(true).describe("Collapse runs of blank lines to a single blank and trim leading/trailing blank lines (paragraph breaks are preserved). Default true."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-corpus-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean a line-oriented text corpus: normalize, dedupe, and filter lines",
    skill(
        description = "Clean a raw, line-oriented text corpus (one record/sentence per line) in one deterministic pass: split on \\n / \\r\\n / \\r, apply Unicode normalization (none/nfc/nfkc), handle whitespace (keep/trim/collapse interior runs), optionally lowercase, filter out lines that are too short (min_chars / min_words) or mostly symbols (max_symbol_ratio) or not the target language (whatlang detection, no model files), deduplicate non-blank lines (exact or case/space-normalized, keeping the first), and collapse runs of blank lines. Returns the cleaned corpus. Ideal for prepping scraped text, word lists, or training data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-corpus-cleaner", |a: Args| {
            run(
                &a.input,
                &a.unicode_form,
                &a.whitespace,
                a.lowercase,
                &a.dedupe,
                &a.language,
                a.min_chars,
                a.min_words,
                a.max_symbol_ratio,
                a.collapse_blank,
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
                    "input":            { "type": "string", "description": "The raw text corpus to clean, one record/sentence per line (e.g. scraped lines or a word list). Lines are split on \\n, \\r\\n, or \\r." },
                    "unicode_form":     { "type": "string", "enum": ["none", "nfc", "nfkc"], "default": "nfc", "description": "Unicode normalization applied to each line before other transforms: 'none' leaves code points untouched; 'nfc' is canonical composition (recommended default); 'nfkc' also folds ligatures (ﬁ→fi), full-width and styled letters, and fractions to plain forms. Default 'nfc'." },
                    "whitespace":       { "type": "string", "enum": ["keep", "trim", "collapse"], "default": "trim", "description": "Per-line whitespace handling: 'keep' leaves it, 'trim' strips only the ends, 'collapse' also squeezes interior runs to one space. Default 'trim'." },
                    "lowercase":        { "type": "boolean", "default": false, "description": "Lowercase every line after normalization. Default false." },
                    "dedupe":           { "type": "string", "enum": ["none", "exact", "normalized"], "default": "exact", "description": "Drop duplicate non-blank lines, keeping the first: 'none' keeps all; 'exact' drops byte-identical repeats; 'normalized' also folds case and interior spacing before comparing. Blank lines are never deduplicated. Default 'exact'." },
                    "language":         { "type": "string", "enum": ["eng", "spa", "fra", "deu", "ita", "por", "nld", "rus", "ara", "cmn", "jpn", "kor", "hin", "any"], "default": "any", "description": "Keep only lines detected as this ISO 639-3 language (eng, spa, fra, deu, ita, por, nld, rus, ara, cmn=Chinese, jpn, kor, hin); 'any' disables the filter. Detection is trigram/script based, no model files; lines too short to detect are kept. Default 'any'." },
                    "min_chars":        { "type": "integer", "minimum": 0, "default": 0, "description": "Drop lines with fewer than this many characters (counted after transforms). 0 keeps all lengths. Default 0." },
                    "min_words":        { "type": "integer", "minimum": 0, "default": 0, "description": "Drop lines with fewer than this many whitespace-separated words. 0 keeps all. Default 0." },
                    "max_symbol_ratio": { "type": "number", "minimum": 0, "maximum": 1, "default": 1.0, "description": "Drop a line if the ratio of symbol characters (non-alphanumeric, non-space) to visible characters exceeds this (0.0–1.0). 1.0 keeps every line; e.g. 0.5 drops lines that are mostly punctuation. Default 1.0." },
                    "collapse_blank":   { "type": "boolean", "default": true, "description": "Collapse runs of blank lines to a single blank and trim leading/trailing blank lines (paragraph breaks are preserved). Default true." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
