//! gizza-ai/csv-date-normalizer — chat skill block on the shared tool abstraction.
//! Rewrites the date/time values in one or more CSV columns into a single
//! canonical format (ISO 8601 by default), inferring day-first vs month-first
//! per column from the rows that can only be read one way. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`. Pure compute — nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    columns: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    custom_format: String,
    #[serde(default)]
    date_order: String,
    #[serde(default = "default_pivot")]
    year_pivot: i64,
    #[serde(default = "default_true")]
    excel_serial: bool,
    #[serde(default)]
    on_error: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_pivot() -> i64 {
    68
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV/delimited table to normalize, as text. Only the chosen date columns are rewritten — every other column, the header row, the quoting and the row order are passed through unchanged. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("columns")
                .default("auto")
                .describe("Which columns to normalize: a comma-separated list of header names and/or 0-based indexes ('joined', 'start,2'), or 'auto' (default) to detect them. Auto-detection claims a column when at least 60% of its non-blank cells parse as a WRITTEN date, and deliberately never claims a column of bare numbers — so Unix timestamps and Excel serials must be named explicitly."),
        )
        .param(
            Param::enumv(
                "format",
                [
                    "iso-auto",
                    "iso-date",
                    "iso-datetime",
                    "iso-utc",
                    "unix-seconds",
                    "unix-millis",
                    "us-date",
                    "eu-date",
                    "sql",
                    "compact",
                    "rfc2822",
                    "custom",
                ],
            )
            .default("iso-auto")
            .describe("Target output format. 'iso-auto' (default) keeps each value's precision — 2024-01-15 for a date-only cell, 2024-01-15T10:30:00Z when the cell carried a time; 'iso-date' -> 2024-01-15; 'iso-datetime' -> 2024-01-15T10:30:00 (offset dropped); 'iso-utc' -> 2024-01-15T05:00:00Z (shifted to UTC); 'unix-seconds' / 'unix-millis' -> epoch integers; 'us-date' -> 01/15/2024; 'eu-date' -> 15/01/2024; 'sql' -> 2024-01-15 10:30:00; 'compact' -> 20240115; 'rfc2822' -> Mon, 15 Jan 2024 10:30:00 +0000; 'custom' uses custom_format."),
        )
        .param(
            Param::string("custom_format")
                .default("")
                .describe("The strftime/chrono pattern used when format is 'custom' — for example '%d %B %Y' gives 15 January 2024, '%b %-d, %Y' gives Jan 15, 2024, '%Y/%m/%d %H:%M' gives 2024/01/15 10:30. Required (and only read) for format = 'custom'; an unknown specifier is rejected rather than emitted literally."),
        )
        .param(
            Param::enumv("date_order", ["auto", "day-first", "month-first"])
                .default("auto")
                .describe("How to read an all-numeric date whose two leading parts are both 12 or less, like 03/04/2024. 'auto' (default) decides PER COLUMN from the rows that can only be one thing — a single 25/12/2021 in the column proves day-first for every other value in it — and falls back to month-first when the column holds no such proof, reporting that as the reason. 'day-first' reads 03/04/2024 as 4 March; 'month-first' reads it as 3 April. Values that settle themselves (ISO, written month names) ignore this setting."),
        )
        .param(
            Param::integer("year_pivot")
                .default(68)
                .min(0.0)
                .max(99.0)
                .describe("Century cut-off for two-digit years: a value at or below the pivot becomes 20xx, above it becomes 19xx. The default 68 is the POSIX/Excel rule, so 68 -> 2068 and 69 -> 1969. Set it to 99 to force every two-digit year into the 2000s, or to 0 to force them all into the 1900s."),
        )
        .param(
            Param::boolean("excel_serial")
                .default(true)
                .describe("When true (default), a bare number between 1 and 2958465 in a named column is read as an Excel 1900-system serial (45000 -> 2023-03-15), with a fractional part becoming the time of day. Turn it off when such a column holds real numbers — quantities, ids, amounts — that must not be reinterpreted as dates. Unix epoch seconds/milliseconds are recognised regardless of this flag, by magnitude."),
        )
        .param(
            Param::enumv("on_error", ["keep", "blank", "error"])
                .default("keep")
                .describe("What to do with a cell that cannot be read as a date: 'keep' (default) leaves the original text in place and counts it in the report, 'blank' empties the cell, 'error' aborts the whole run naming the first offending row, line and column. Blank cells are never touched under any setting."),
        )
        .param(
            Param::boolean("has_header")
                .default(true)
                .describe("When true (default), row 1 is a header: it is never rewritten and its names can be used in 'columns'. Turn it off for a headerless table — every row is then treated as data and columns must be given as 0-based indexes."),
        )
        .param(
            Param::string("delimiter")
                .default("auto")
                .describe("Field separator: 'auto' (default) sniffs it from the first non-blank line, or give a name ('comma', 'tab', 'semicolon', 'pipe') or any single character. The output uses the same separator as the input."),
        )
        .param(
            Param::enumv("output", ["csv", "report", "json"])
                .default("csv")
                .describe("What to return: 'csv' (default) is the rewritten table; 'report' is a plain-text audit — per column the day/month order used and WHY, plus counts of converted / already-normalized / unreadable / blank cells and the first 20 unreadable values with their row and line numbers; 'json' is the same audit as a JSON object with the rewritten table under 'csv'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-date-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite mixed CSV date columns into one format, inferring day-first vs month-first per column.",
    skill(
        description = "Normalize the dates in one or more CSV columns to a single format. A column holding 2021-06-01, 06/15/2021, 15 Jan 2024, Tue, 15 Jan 2024 10:30:00 +0000, 45000 and 1700000000 comes back as one consistent ISO 8601 column. The hard part is 03/04/2024, and this tool does not guess per cell: it reads the whole column first, uses the rows that can only be one thing (a day above 12) to settle day-first vs month-first for that column, and reports which it chose and why — including a 'conflict' verdict when the same column proves BOTH orders. Recognised inputs include ISO 8601 with T/Z/offsets, slash- dot- and dash-separated numerics, written month names with ordinals (Jan 15th, 2024), leading weekday names, 12-hour times with am/pm, two-digit years via a configurable century pivot, compact 20240115 / 20240115103000, Unix epoch seconds and milliseconds, and Excel 1900-system serials. columns takes header names and/or 0-based indexes, or 'auto' to detect date columns (auto never claims a column of bare numbers, so timestamps and Excel serials must be named). format is iso-auto (default, keeps each value's precision), iso-date, iso-datetime, iso-utc, unix-seconds, unix-millis, us-date, eu-date, sql, compact, rfc2822, or custom with a strftime custom_format. date_order forces day-first or month-first; year_pivot sets the two-digit-year cut-off (68 by default); excel_serial can be turned off so numeric columns are left alone; on_error keeps, blanks, or errors on an unreadable cell; has_header and delimiter describe the table; output is csv, a plain-text report, or json. Only the named columns are touched — other columns, the header, quoting and row order are unchanged, and impossible calendar dates like 2021-02-30 are refused rather than rolled over. Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-date-normalizer", |a: Args| {
            gizza_ai_csv_date_normalizer_core::run(
                &a.data,
                &a.columns,
                &a.format,
                &a.custom_format,
                &a.date_order,
                a.year_pivot,
                a.excel_serial,
                &a.on_error,
                a.has_header,
                &a.delimiter,
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
    /// reviewed. Authored 2026-08-16 for the initial csv-date-normalizer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV/delimited table to normalize, as text. Only the chosen date columns are rewritten — every other column, the header row, the quoting and the row order are passed through unchanged. Max 5,000,000 bytes." },
                    "columns": { "type": "string", "default": "auto", "description": "Which columns to normalize: a comma-separated list of header names and/or 0-based indexes ('joined', 'start,2'), or 'auto' (default) to detect them. Auto-detection claims a column when at least 60% of its non-blank cells parse as a WRITTEN date, and deliberately never claims a column of bare numbers — so Unix timestamps and Excel serials must be named explicitly." },
                    "format": { "type": "string", "enum": ["iso-auto", "iso-date", "iso-datetime", "iso-utc", "unix-seconds", "unix-millis", "us-date", "eu-date", "sql", "compact", "rfc2822", "custom"], "default": "iso-auto", "description": "Target output format. 'iso-auto' (default) keeps each value's precision — 2024-01-15 for a date-only cell, 2024-01-15T10:30:00Z when the cell carried a time; 'iso-date' -> 2024-01-15; 'iso-datetime' -> 2024-01-15T10:30:00 (offset dropped); 'iso-utc' -> 2024-01-15T05:00:00Z (shifted to UTC); 'unix-seconds' / 'unix-millis' -> epoch integers; 'us-date' -> 01/15/2024; 'eu-date' -> 15/01/2024; 'sql' -> 2024-01-15 10:30:00; 'compact' -> 20240115; 'rfc2822' -> Mon, 15 Jan 2024 10:30:00 +0000; 'custom' uses custom_format." },
                    "custom_format": { "type": "string", "default": "", "description": "The strftime/chrono pattern used when format is 'custom' — for example '%d %B %Y' gives 15 January 2024, '%b %-d, %Y' gives Jan 15, 2024, '%Y/%m/%d %H:%M' gives 2024/01/15 10:30. Required (and only read) for format = 'custom'; an unknown specifier is rejected rather than emitted literally." },
                    "date_order": { "type": "string", "enum": ["auto", "day-first", "month-first"], "default": "auto", "description": "How to read an all-numeric date whose two leading parts are both 12 or less, like 03/04/2024. 'auto' (default) decides PER COLUMN from the rows that can only be one thing — a single 25/12/2021 in the column proves day-first for every other value in it — and falls back to month-first when the column holds no such proof, reporting that as the reason. 'day-first' reads 03/04/2024 as 4 March; 'month-first' reads it as 3 April. Values that settle themselves (ISO, written month names) ignore this setting." },
                    "year_pivot": { "type": "integer", "minimum": 0, "maximum": 99, "default": 68, "description": "Century cut-off for two-digit years: a value at or below the pivot becomes 20xx, above it becomes 19xx. The default 68 is the POSIX/Excel rule, so 68 -> 2068 and 69 -> 1969. Set it to 99 to force every two-digit year into the 2000s, or to 0 to force them all into the 1900s." },
                    "excel_serial": { "type": "boolean", "default": true, "description": "When true (default), a bare number between 1 and 2958465 in a named column is read as an Excel 1900-system serial (45000 -> 2023-03-15), with a fractional part becoming the time of day. Turn it off when such a column holds real numbers — quantities, ids, amounts — that must not be reinterpreted as dates. Unix epoch seconds/milliseconds are recognised regardless of this flag, by magnitude." },
                    "on_error": { "type": "string", "enum": ["keep", "blank", "error"], "default": "keep", "description": "What to do with a cell that cannot be read as a date: 'keep' (default) leaves the original text in place and counts it in the report, 'blank' empties the cell, 'error' aborts the whole run naming the first offending row, line and column. Blank cells are never touched under any setting." },
                    "has_header": { "type": "boolean", "default": true, "description": "When true (default), row 1 is a header: it is never rewritten and its names can be used in 'columns'. Turn it off for a headerless table — every row is then treated as data and columns must be given as 0-based indexes." },
                    "delimiter": { "type": "string", "default": "auto", "description": "Field separator: 'auto' (default) sniffs it from the first non-blank line, or give a name ('comma', 'tab', 'semicolon', 'pipe') or any single character. The output uses the same separator as the input." },
                    "output": { "type": "string", "enum": ["csv", "report", "json"], "default": "csv", "description": "What to return: 'csv' (default) is the rewritten table; 'report' is a plain-text audit — per column the day/month order used and WHY, plus counts of converted / already-normalized / unreadable / blank cells and the first 20 unreadable values with their row and line numbers; 'json' is the same audit as a JSON object with the rewritten table under 'csv'." }
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
