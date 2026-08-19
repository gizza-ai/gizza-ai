//! gizza-ai/csv-null-standardizer — chat skill block on the shared tool abstraction.
//! Rewrites every missing-value token in a CSV/delimited table (NA, N/A, #N/A,
//! NULL, NaN, None, <NA>, -, ?, blank cells, …) into ONE chosen representation, so
//! a downstream loader has a single rule to configure. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`. Pure compute — nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_na_tokens")]
    na_tokens: String,
    #[serde(default)]
    replace_with: String,
    #[serde(default = "default_true")]
    blank_is_missing: bool,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    quote_style: String,
}

fn default_na_tokens() -> String {
    gizza_ai_csv_null_standardizer_core::DEFAULT_NA_TOKENS.to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The CSV/delimited table to standardize, as text. Quoted fields with embedded separators or newlines (RFC 4180) are preserved. Rows may be ragged — a short row stays short. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input."),
        )
        .param(
            Param::string("na_tokens")
                .default(gizza_ai_csv_null_standardizer_core::DEFAULT_NA_TOKENS)
                .describe("Comma-separated tokens that count as missing, e.g. 'NA,N/A,NULL,NaN,-'. Defaults to the built-in vocabulary 'NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?'. Edit the list to add your own sentinels (e.g. '-999'). Set it empty to standardize ONLY blank cells."),
        )
        .param(
            Param::string("replace_with")
                .default("")
                .describe("The single representation written for every missing cell, e.g. 'NULL', 'NA', 'NaN', or '\\N' for a Postgres COPY load. Default is an empty string, which leaves the cell blank."),
        )
        .param(
            Param::boolean("blank_is_missing")
                .default(true)
                .describe("When true (default), a cell that is empty or whitespace-only also counts as missing and is rewritten. Turn it off to convert only the listed tokens and leave already-blank cells alone."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When false (default), tokens match regardless of case, so one 'NULL' entry also catches 'null' and 'Null'. Turn it on to require an exact-case match."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("When true (default), whitespace around a cell is ignored while matching, so ' NA ' still counts as missing. Cells that are NOT missing are always copied verbatim, padding included — this option never rewrites real values."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row is a header: it is copied through untouched, so a column literally named 'NA' survives, and it supplies the names used by the columns option."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Restrict the rewrite to specific columns: a comma-separated list of column names (needs header = true) or 1-based positions, e.g. 'score,notes' or '2,4'. Default (empty) standardizes every column."),
        )
        .param(
            Param::enumv("quote_style", ["minimal", "always", "never"])
                .default("minimal")
                .describe("Output quoting: 'minimal' (default) quotes only fields that need it; 'always' quotes every field, the replacement token included, for readers that expect uniformly quoted CSV; 'never' emits bare fields, which is compact but can produce ambiguous CSV when values contain the separator. Keep 'minimal' when replace_with is a loader sentinel such as '\\N', because a quoted sentinel is read as a literal string."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-null-standardizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite assorted CSV missing-value tokens (NA, N/A, null, -, NaN, blanks) into one chosen representation.",
    skill(
        description = "Standardize missing-value tokens in a CSV/delimited table. Real-world exports mix NA, N/A, #N/A, NULL, NaN, None, <NA>, a bare dash and empty cells in the same file; this rewrites every one of them into a single representation you choose, so a loader or dataframe needs one rule instead of a dozen. Non-missing cells are copied verbatim (no trimming, no re-typing), the header row is never rewritten, and the field separator round-trips unchanged. delimiter accepts 'auto' (sniffed from the first line), a single char, or 'comma'/'tab'/'semicolon'/'pipe'. na_tokens is the comma-separated missing vocabulary (default 'NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?'; add sentinels like '-999'; empty = blanks only). replace_with is what every missing cell becomes (default empty = a blank cell; try 'NULL', 'NaN', or '\\N' for Postgres COPY). blank_is_missing (default on) includes empty/whitespace-only cells; case_sensitive (default off) requires exact case; trim (default on) ignores padding while matching; header (default on) protects row 1; columns limits the rewrite to named or 1-based columns; quote_style is 'minimal', 'always', or 'never'. This normalizes the TOKEN — it never invents a value, so use an imputer if you need means/medians filled in. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-null-standardizer", |a: Args| {
            gizza_ai_csv_null_standardizer_core::standardize(
                &a.input,
                &a.delimiter,
                &a.na_tokens,
                &a.replace_with,
                a.blank_is_missing,
                a.case_sensitive,
                a.trim,
                a.header,
                &a.columns,
                &a.quote_style,
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
    /// reviewed. Authored 2026-08-12 for the initial csv-null-standardizer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The CSV/delimited table to standardize, as text. Quoted fields with embedded separators or newlines (RFC 4180) are preserved. Rows may be ragged — a short row stays short. Max 5,000,000 bytes." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: 'auto' to sniff it from the first line, a single character, or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). The output uses the same separator as the input." },
                    "na_tokens": { "type": "string", "default": "NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?", "description": "Comma-separated tokens that count as missing, e.g. 'NA,N/A,NULL,NaN,-'. Defaults to the built-in vocabulary 'NA,N/A,N.A.,#N/A,#N/A N/A,#NA,NULL,NIL,NaN,None,<NA>,-,--,?'. Edit the list to add your own sentinels (e.g. '-999'). Set it empty to standardize ONLY blank cells." },
                    "replace_with": { "type": "string", "default": "", "description": "The single representation written for every missing cell, e.g. 'NULL', 'NA', 'NaN', or '\\N' for a Postgres COPY load. Default is an empty string, which leaves the cell blank." },
                    "blank_is_missing": { "type": "boolean", "default": true, "description": "When true (default), a cell that is empty or whitespace-only also counts as missing and is rewritten. Turn it off to convert only the listed tokens and leave already-blank cells alone." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When false (default), tokens match regardless of case, so one 'NULL' entry also catches 'null' and 'Null'. Turn it on to require an exact-case match." },
                    "trim": { "type": "boolean", "default": true, "description": "When true (default), whitespace around a cell is ignored while matching, so ' NA ' still counts as missing. Cells that are NOT missing are always copied verbatim, padding included — this option never rewrites real values." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row is a header: it is copied through untouched, so a column literally named 'NA' survives, and it supplies the names used by the columns option." },
                    "columns": { "type": "string", "default": "", "description": "Restrict the rewrite to specific columns: a comma-separated list of column names (needs header = true) or 1-based positions, e.g. 'score,notes' or '2,4'. Default (empty) standardizes every column." },
                    "quote_style": { "type": "string", "enum": ["minimal", "always", "never"], "default": "minimal", "description": "Output quoting: 'minimal' (default) quotes only fields that need it; 'always' quotes every field, the replacement token included, for readers that expect uniformly quoted CSV; 'never' emits bare fields, which is compact but can produce ambiguous CSV when values contain the separator. Keep 'minimal' when replace_with is a loader sentinel such as '\\N', because a quoted sentinel is read as a literal string." }
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
