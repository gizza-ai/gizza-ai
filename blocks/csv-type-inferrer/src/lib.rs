//! gizza-ai/csv-type-inferrer — chat skill block on the shared tool abstraction.
//!
//! Sniffs a CSV's delimiter/quote dialect, detects a header row, infers a type
//! for every column, and emits a JSON schema report and/or typed JSON records.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const DEFAULT_NULL_TOKENS: &str = "NA,N/A,NULL,null,None,nan";

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    headers: String,
    #[serde(default)]
    null_tokens: String,
    /// Detect date/datetime columns (default true).
    #[serde(default = "default_true")]
    date_detection: bool,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV text to analyze (paste the file contents, including the header row if there is one)."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("Field delimiter. 'auto' (default) sniffs it from the data by testing comma, semicolon, tab and pipe for the most consistent column count; otherwise force one of comma, tab, semicolon, pipe."),
        )
        .param(
            Param::enumv("headers", ["auto", "present", "absent"])
                .default("auto")
                .describe("Whether the first row holds column names. 'auto' (default) detects it (a text first row over typed data rows = header); 'present' forces the first row to be names; 'absent' generates column_1, column_2, ... and treats every row as data."),
        )
        .param(
            Param::string("null_tokens")
                .default(DEFAULT_NULL_TOKENS)
                .describe("Comma-separated tokens (in addition to the empty string) treated as missing/null when inferring types, e.g. 'NA,N/A,NULL'. Matching is exact and case-sensitive."),
        )
        .param(
            Param::boolean("date_detection")
                .default(true)
                .describe("Recognize date columns (YYYY-MM-DD, MM/DD/YYYY, DD/MM/YYYY and dash/slash variants) and datetime columns (YYYY-MM-DD[ T]HH:MM:SS, optional Z). Default true; set false to keep such columns as strings."),
        )
        .param(
            Param::enumv("output", ["both", "schema", "records"])
                .default("both")
                .describe("What to emit. 'both' (default) = a schema report (dialect + per-column types/counts) plus the typed records; 'schema' = the report only; 'records' = just the array of type-coerced JSON objects."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-type-inferrer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sniff a CSV's dialect and infer per-column types as JSON.",
    skill(
        description = "Sniff a CSV's delimiter/quote dialect, detect whether the first row is a header, infer a type for every column (int, float, bool, date, datetime, string, or empty), and emit a JSON report plus typed JSON records. delimiter='auto' (default) sniffs comma/semicolon/tab/pipe; headers='auto' (default) detects the header row; null_tokens lists strings treated as missing (default 'NA,N/A,NULL,null,None,nan'); date_detection=true (default) recognizes common date/datetime formats; output='both' (default) returns the schema and the type-coerced records, 'schema' just the report, 'records' just the array. Zero-padded integers like '007' stay strings to preserve codes; whitespace around cells is trimmed.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-type-inferrer", |a: Args| {
            let null_tokens = if a.null_tokens.trim().is_empty() {
                DEFAULT_NULL_TOKENS
            } else {
                a.null_tokens.as_str()
            };
            gizza_ai_csv_type_inferrer_core::infer(
                &a.data,
                &a.delimiter,
                &a.headers,
                null_tokens,
                a.date_detection,
                &a.output,
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
                    "data": { "type": "string", "description": "The CSV text to analyze (paste the file contents, including the header row if there is one)." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "Field delimiter. 'auto' (default) sniffs it from the data by testing comma, semicolon, tab and pipe for the most consistent column count; otherwise force one of comma, tab, semicolon, pipe." },
                    "headers": { "type": "string", "enum": ["auto", "present", "absent"], "default": "auto", "description": "Whether the first row holds column names. 'auto' (default) detects it (a text first row over typed data rows = header); 'present' forces the first row to be names; 'absent' generates column_1, column_2, ... and treats every row as data." },
                    "null_tokens": { "type": "string", "default": "NA,N/A,NULL,null,None,nan", "description": "Comma-separated tokens (in addition to the empty string) treated as missing/null when inferring types, e.g. 'NA,N/A,NULL'. Matching is exact and case-sensitive." },
                    "date_detection": { "type": "boolean", "default": true, "description": "Recognize date columns (YYYY-MM-DD, MM/DD/YYYY, DD/MM/YYYY and dash/slash variants) and datetime columns (YYYY-MM-DD[ T]HH:MM:SS, optional Z). Default true; set false to keep such columns as strings." },
                    "output": { "type": "string", "enum": ["both", "schema", "records"], "default": "both", "description": "What to emit. 'both' (default) = a schema report (dialect + per-column types/counts) plus the typed records; 'schema' = the report only; 'records' = just the array of type-coerced JSON objects." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
