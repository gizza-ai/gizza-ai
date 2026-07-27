//! gizza-ai/date-column-validate — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool { true }
fn default_max_issues() -> u64 { 50 }

#[derive(Deserialize)]
struct Args {
    data: String,
    column: String,
    #[serde(default)]
    preset: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_true")]
    allow_blank: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_max_issues")]
    max_issues: u64,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("CSV data to check. Paste rows as text; quoted fields and quoted newlines are supported. The first row is treated as a header by default."),
        )
        .param(
            Param::string("column")
                .required()
                .describe("Which column to validate: a header name (for example `joined`) when the first row is a header, or a 0-based column index (for example `1` for the second column). A numeric value is always read as a 0-based index."),
        )
        .param(
            Param::enumv("preset", ["iso-date", "us-date", "eu-date", "iso-datetime", "rfc3339", "custom"])
                .default("iso-date")
                .describe("Date format to check against. `iso-date` = YYYY-MM-DD (default), `us-date` = MM/DD/YYYY, `eu-date` = DD/MM/YYYY, `iso-datetime` = YYYY-MM-DDThh:mm:ss, `rfc3339` = full RFC 3339 date-time. Choose `custom` to supply your own pattern in `format`."),
        )
        .param(
            Param::string("format")
                .default("%Y-%m-%d")
                .describe("Custom chrono/strftime pattern, used only when preset=custom (for example `%d-%b-%Y` for `01-Jun-2021`, `%Y%m%d`, or `%H:%M:%S`). Common specifiers: %Y year, %m month, %d day, %b short month name, %H:%M:%S time. Ignored for the non-custom presets."),
        )
        .param(
            Param::boolean("has_header")
                .default(true)
                .describe("Treat the first row as a header naming the columns. Default true; set false to select the column by 0-based index only."),
        )
        .param(
            Param::boolean("allow_blank")
                .default(true)
                .describe("Treat blank cells as valid (skipped). Default true; set false to report every blank cell in the column as invalid."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("CSV delimiter. `auto` (default) detects comma, tab, semicolon, or pipe from the first non-blank line."),
        )
        .param(
            Param::integer("max_issues")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of invalid values to list (1-1000, default 50). The summary still reports the full invalid count."),
        )
        .param(
            Param::enumv("output", ["text", "json"])
                .default("text")
                .describe("Output format: `text` for a readable report (default) or `json` for a structured report with total_checked, valid, invalid, truncated, and invalid_rows."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/date-column-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check every value in a CSV date column against a chosen format.",
    skill(
        description = "Validate that every value in one CSV date column parses against a chosen date format. Pick the column by header name or 0-based index, then check it against a preset (ISO YYYY-MM-DD, US MM/DD/YYYY, EU DD/MM/YYYY, ISO date-time, or RFC 3339) or a custom chrono/strftime pattern. Impossible calendar dates (month 13, Feb 30, bad leap days) are rejected, not just malformed shapes. Reports total checked, valid count, invalid count, and a capped list of offending rows with their line number, value, and reason. Supports delimiter auto-detection, header or headerless CSV, optional blank-cell rejection, text output, and JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "date-column-validate", |a: Args| {
            gizza_ai_date_column_validate_core::run(
                &a.data,
                &a.column,
                &a.preset,
                &a.format,
                a.has_header,
                a.allow_blank,
                &a.delimiter,
                a.max_issues as usize,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
