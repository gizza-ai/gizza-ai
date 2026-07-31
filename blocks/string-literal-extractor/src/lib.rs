//! gizza-ai/string-literal-extractor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and calls the shared core
//! tokenizer in `core::extract`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_quotes")]
    quotes: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    decode_escapes: bool,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    min_length: i64,
    #[serde(default)]
    line_numbers: bool,
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_quotes() -> String {
    "all".to_string()
}
fn default_format() -> String {
    "list".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("code").required().describe("The source code to scan for string literals. Paste a snippet or a whole file; only strings outside comments are extracted."))
        .param(Param::enumv("language", ["auto", "python", "javascript", "typescript", "java", "csharp", "c", "cpp", "go", "rust", "php", "ruby", "shell"]).default("auto").describe("Language whose string/comment/escape rules to apply. 'auto' guesses Python vs a generic C/JS profile from the code."))
        .param(Param::enumv("quotes", ["all", "double", "single", "backtick"]).default("all").describe("Which quote style to keep: all, or only double-quoted, single-quoted, or backtick/template literals."))
        .param(Param::enumv("format", ["list", "json", "csv"]).default("list").describe("Output shape: a plain list of the values, JSON objects with line/column/quote/value, or CSV rows with those columns."))
        .param(Param::boolean("decode_escapes").default(false).describe("Decode escape sequences (\\n, \\t, \\xNN, \\uXXXX, …) into their actual characters instead of leaving them literal. Ignored for raw strings."))
        .param(Param::boolean("unique").default(false).describe("Drop duplicate values, keeping the first occurrence of each."))
        .param(Param::integer("min_length").default(0).min(0.0).describe("Skip literals shorter than this many characters. 0 keeps every string."))
        .param(Param::boolean("line_numbers").default(false).describe("Append the source line as [Ln] to each value in list output."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/string-literal-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every string literal from pasted source code, skipping comments",
    skill(
        description = "Pull every string literal out of a block of source code with a language-aware tokenizer that skips strings inside line and block comments, treats single quotes as character literals where the language does (C, Java, Rust, Go, C#), and understands Python triple-quoted strings, Go raw backticks, and Rust raw strings. Filter by quote style, drop duplicates, filter by minimum length, optionally decode escape sequences, and return the results as a plain list, JSON, or CSV — each literal carrying its source line and column. Set 'language' to auto to guess Python vs a generic C/JS profile.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "string-literal-extractor", |a: Args| {
            gizza_ai_string_literal_extractor_core::extract(
                &a.code,
                &a.language,
                &a.quotes,
                a.decode_escapes,
                a.unique,
                a.min_length,
                &a.format,
                a.line_numbers,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "The source code to scan for string literals. Paste a snippet or a whole file; only strings outside comments are extracted." },
                "language": { "type": "string", "enum": ["auto", "python", "javascript", "typescript", "java", "csharp", "c", "cpp", "go", "rust", "php", "ruby", "shell"], "default": "auto", "description": "Language whose string/comment/escape rules to apply. 'auto' guesses Python vs a generic C/JS profile from the code." },
                "quotes": { "type": "string", "enum": ["all", "double", "single", "backtick"], "default": "all", "description": "Which quote style to keep: all, or only double-quoted, single-quoted, or backtick/template literals." },
                "format": { "type": "string", "enum": ["list", "json", "csv"], "default": "list", "description": "Output shape: a plain list of the values, JSON objects with line/column/quote/value, or CSV rows with those columns." },
                "decode_escapes": { "type": "boolean", "default": false, "description": "Decode escape sequences (\\n, \\t, \\xNN, \\uXXXX, …) into their actual characters instead of leaving them literal. Ignored for raw strings." },
                "unique": { "type": "boolean", "default": false, "description": "Drop duplicate values, keeping the first occurrence of each." },
                "min_length": { "type": "integer", "default": 0, "minimum": 0, "description": "Skip literals shorter than this many characters. 0 keeps every string." },
                "line_numbers": { "type": "boolean", "default": false, "description": "Append the source line as [Ln] to each value in list output." }
            },
            "required": ["code"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
