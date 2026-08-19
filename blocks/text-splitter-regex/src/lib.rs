//! gizza-ai/text-splitter-regex — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    field_pattern: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    remove_empty: bool,
    #[serde(default)]
    max_splits: f64,
    #[serde(default)]
    output: String,
    #[serde(default = "default_separator")]
    separator: String,
}

fn default_separator() -> String {
    ", ".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to split."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("The separator as a regular expression (Rust regex syntax) — everything between matches becomes a part. Examples: \\s+ (runs of whitespace), [,;|] (any of several delimiters), \\n{2,} (blank lines / paragraphs), \\s*,\\s* (commas with optional spaces)."),
        )
        .param(
            Param::string("field_pattern")
                .default("")
                .describe("Optional second regular expression that splits each row into fields, turning the input into a table (e.g. rows on \\n and fields on \\s*:\\s*). Blank (the default) splits into rows only."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match both patterns case-insensitively (regex flag i). Default false."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe("Let ^ and $ match at line boundaries instead of only the start/end of the text (regex flag m). Default false."),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("Let . also match newline characters, so a separator can span lines (regex flag s). Default false."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from every part. Default false."),
        )
        .param(
            Param::boolean("remove_empty")
                .default(false)
                .describe("Drop empty parts (after trimming, if trim is on) — useful for leading, trailing or repeated separators. Default false."),
        )
        .param(
            Param::integer("max_splits")
                .default(0)
                .min(0.0)
                .describe("Stop after this many splits into rows and keep the rest as the final row (0 = unlimited, the default). Field splitting is never capped. Example: max_splits=1 on 'key: some: value' with pattern ':\\s*' gives 'key' and 'some: value'."),
        )
        .param(
            Param::enumv("output", ["lines", "json", "csv", "tsv", "numbered", "separator"])
                .default("lines")
                .describe("How to render the parts. 'lines' (default) = one part per line (fields joined by a tab); 'json' = a JSON array of strings, or of arrays when field_pattern is set; 'csv' = RFC-4180 CSV; 'tsv' = tab-separated; 'numbered' = '1. part' per line; 'separator' = joined by the separator parameter."),
        )
        .param(
            Param::string("separator")
                .default(", ")
                .describe("The string used to join parts when output='separator' (default ', '). The escapes \\n, \\t, \\r and \\\\ are recognised, so \\n\\n puts a blank line between parts. Ignored for every other output format."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-splitter-regex",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split text into rows or fields using a regular expression as the delimiter.",
    skill(
        description = "Split text into parts using a regular expression as the SEPARATOR (the inverse of matching): everything between matches of `pattern` becomes a part, which handles multi-character, mixed and repeated-whitespace delimiters that a literal split cannot. Set field_pattern to split every row again into fields and get a real table. Regex flags ignore_case, multiline and dotall apply to both patterns; trim and remove_empty clean up the parts; max_splits caps the number of row splits and keeps the remainder intact. Render as one-per-line (default), json, csv, tsv, numbered, or joined by a custom separator. Input is capped at 200,000 characters and 100,000 parts.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-splitter-regex", |a: Args| {
            let max_splits = if a.max_splits.is_finite() && a.max_splits > 0.0 {
                a.max_splits as usize
            } else {
                0
            };
            gizza_ai_text_splitter_regex_core::split(
                &a.text,
                &a.pattern,
                &a.field_pattern,
                a.ignore_case,
                a.multiline,
                a.dotall,
                a.trim,
                a.remove_empty,
                max_splits,
                &a.output,
                &a.separator,
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
                    "text": { "type": "string", "description": "The text to split." },
                    "pattern": { "type": "string", "description": "The separator as a regular expression (Rust regex syntax) — everything between matches becomes a part. Examples: \\s+ (runs of whitespace), [,;|] (any of several delimiters), \\n{2,} (blank lines / paragraphs), \\s*,\\s* (commas with optional spaces)." },
                    "field_pattern": { "type": "string", "default": "", "description": "Optional second regular expression that splits each row into fields, turning the input into a table (e.g. rows on \\n and fields on \\s*:\\s*). Blank (the default) splits into rows only." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match both patterns case-insensitively (regex flag i). Default false." },
                    "multiline": { "type": "boolean", "default": false, "description": "Let ^ and $ match at line boundaries instead of only the start/end of the text (regex flag m). Default false." },
                    "dotall": { "type": "boolean", "default": false, "description": "Let . also match newline characters, so a separator can span lines (regex flag s). Default false." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from every part. Default false." },
                    "remove_empty": { "type": "boolean", "default": false, "description": "Drop empty parts (after trimming, if trim is on) — useful for leading, trailing or repeated separators. Default false." },
                    "max_splits": { "type": "integer", "default": 0, "minimum": 0, "description": "Stop after this many splits into rows and keep the rest as the final row (0 = unlimited, the default). Field splitting is never capped. Example: max_splits=1 on 'key: some: value' with pattern ':\\s*' gives 'key' and 'some: value'." },
                    "output": { "type": "string", "enum": ["lines", "json", "csv", "tsv", "numbered", "separator"], "default": "lines", "description": "How to render the parts. 'lines' (default) = one part per line (fields joined by a tab); 'json' = a JSON array of strings, or of arrays when field_pattern is set; 'csv' = RFC-4180 CSV; 'tsv' = tab-separated; 'numbered' = '1. part' per line; 'separator' = joined by the separator parameter." },
                    "separator": { "type": "string", "default": ", ", "description": "The string used to join parts when output='separator' (default ', '). The escapes \\n, \\t, \\r and \\\\ are recognised, so \\n\\n puts a blank line between parts. Ignored for every other output format." }
                },
                "required": ["text", "pattern"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
