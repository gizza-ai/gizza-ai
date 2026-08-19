//! gizza-ai/csv-timeline-viewer — chat skill block on the shared tool abstraction.
//! Loads a CSV/TSV/JSONL table of timestamped events and slices it: time range,
//! column conditions, full-text or regex search, sort, projection, paging.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    format: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    time_column: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    tz_offset: f64,
    #[serde(default)]
    search: String,
    #[serde(default)]
    search_fields: String,
    #[serde(default)]
    regex: bool,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    filters: String,
    #[serde(default)]
    sort_by: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_limit")]
    limit: f64,
    #[serde(default)]
    offset: f64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}
fn default_output() -> String {
    "table".to_string()
}
fn default_true() -> bool {
    true
}
fn default_limit() -> f64 {
    100.0
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The event table to view, pasted in full: CSV, TSV, or JSON Lines (one JSON object per line; a whole JSON array of objects is accepted too). Example: `timestamp,level,message` then `2024-06-01T10:00:05Z,ERROR,upstream timeout`. Up to 200000 lines."),
        )
        .param(
            Param::enumv("format", ["auto", "csv", "tsv", "jsonl"])
                .default("auto")
                .describe("Input format. auto (default) reads the first non-blank line and picks jsonl when it starts with { or [, otherwise delimited text. Set csv/tsv/jsonl to override a bad guess."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("Field separator for CSV/TSV input. auto (default) counts commas, semicolons, tabs and pipes outside quotes on the first line and picks the most common. Ignored for jsonl."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row holds column names. Set false for headerless data — columns are then named column1, column2, … and can be referenced by those names or by 1-based index."),
        )
        .param(
            Param::string("time_column")
                .default("")
                .describe("Which column holds the event time: a header name (case-insensitive) or a 1-based index. Empty (default) auto-detects it by header name (timestamp, time, date, created_at, @timestamp, TimeCreated, …) and falls back to the first column whose values actually parse as times."),
        )
        .param(
            Param::string("from")
                .default("")
                .describe("Keep only events at or after this time, inclusive — e.g. 2024-06-01, 2024-06-01T10:00:00Z, or an epoch value. A date with no time means 00:00:00. Empty (default) means no lower bound."),
        )
        .param(
            Param::string("to")
                .default("")
                .describe("Keep only events at or before this time, inclusive — e.g. 2024-06-02 or 2024-06-02T23:59:59Z. A date with no time covers that whole day through 23:59:59.999. Empty (default) means no upper bound."),
        )
        .param(
            Param::number("tz_offset")
                .default(0.0)
                .min(-14.0)
                .max(14.0)
                .describe("Hours that timezone-less timestamps in the data (and in from/to) are offset from UTC, e.g. -5 for US Eastern standard time or 5.5 for India. Values that already carry a Z or ±hh:mm offset are unaffected. Default 0."),
        )
        .param(
            Param::string("search")
                .default("")
                .describe("Full-text search: keep rows where any searched column contains this text, e.g. timeout. Case-insensitive substring by default; set regex=true to treat it as a regular expression such as `job \\d+`. Empty (default) searches nothing away."),
        )
        .param(
            Param::string("search_fields")
                .default("")
                .describe("Comma-separated columns the search looks in, by header name or 1-based index — e.g. message, service. Empty (default) searches every column."),
        )
        .param(
            Param::boolean("regex")
                .default(false)
                .describe("When true, `search` is a regular expression instead of a plain substring. Off by default. An invalid pattern is an error naming the syntax problem."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When true, `search` matches case exactly. Off by default, so ERROR finds error. Column filters using contains/startswith/endswith always ignore case."),
        )
        .param(
            Param::string("filters")
                .default("")
                .describe("Column conditions, ONE PER LINE, all of which must hold: `<column> <op> <value>` with op one of == != < <= > >= contains !contains startswith endswith matches. Example: `level == ERROR` on one line and `service contains work` on the next. Comparison is numeric when both sides are numbers, otherwise text; `matches` takes a regular expression. Empty (default) keeps every row."),
        )
        .param(
            Param::string("sort_by")
                .default("")
                .describe("Column to sort by, as a header name or 1-based index. Empty (default) sorts by the timestamp column; rows whose timestamp does not parse are placed last. Non-time columns sort numerically when both cells are numbers, otherwise alphabetically."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Sort direction: asc (oldest or smallest first, the default) or desc (newest or largest first)."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Comma-separated columns to show, in this order, by header name or 1-based index — e.g. timestamp, level, message. Empty (default) shows every column."),
        )
        .param(
            Param::integer("limit")
                .default(100)
                .min(1.0)
                .max(100_000.0)
                .describe("How many matching rows to show, i.e. the page size (1-100000, default 100). The match count in the footer always reports the full total, so a trim is never silent."),
        )
        .param(
            Param::integer("offset")
                .default(0)
                .min(0.0)
                .max(1_000_000.0)
                .describe("How many matching rows to skip before the page starts (default 0). Use with limit to page: offset=100 with limit=100 is the second page."),
        )
        .param(
            Param::enumv("output", ["table", "csv", "json", "jsonl", "summary"])
                .default("table")
                .describe("Result shape: table (default, an aligned grid with a # source-row-number column and a match-count footer), csv, json (array of objects), jsonl (one object per line), or summary (row counts, detected time column, first/last event and an activity histogram over the whole match set, ignoring limit/offset)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-timeline-viewer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort, filter, search and time-slice a CSV/JSONL event timeline",
    skill(
        description = "View a large CSV, TSV, or JSON Lines table of timestamped events: filter it to a time range, search across every column, apply per-column conditions, sort, pick columns, and page through the result. The timestamp column is auto-detected by header name (timestamp, time, date, created_at, TimeCreated, …) or by which column's values actually parse as times, and ISO 8601, `YYYY-MM-DD HH:MM:SS`, Apache `01/Jun/2024:10:00:00 +0000`, and bare epoch seconds/millis/micros/nanos are all understood; tz_offset interprets timezone-less values. from/to bound an inclusive range (a bare date covers the whole day), search does case-insensitive substring or opt-in regex matching over every column or just search_fields, and filters applies one `<column> <op> <value>` condition per line (== != < <= > >= contains !contains startswith endswith matches), AND-ed together. sort_by/order sort (by time when blank), columns projects, limit/offset page. Output is table (aligned grid with source row numbers and a match-count footer), csv, json, jsonl, or summary (counts, first/last event, and an activity histogram that reveals spikes). Up to 200000 input lines. Runs locally — the data never leaves the machine.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-timeline-viewer", |a: Args| {
            gizza_ai_csv_timeline_viewer_core::view(
                &a.data,
                &a.format,
                &a.delimiter,
                a.header,
                &a.time_column,
                &a.from,
                &a.to,
                a.tz_offset,
                &a.search,
                &a.search_fields,
                a.regex,
                a.case_sensitive,
                &a.filters,
                &a.sort_by,
                &a.order,
                &a.columns,
                a.limit.round().max(0.0) as u32,
                a.offset.round().max(0.0) as u32,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The event table to view, pasted in full: CSV, TSV, or JSON Lines (one JSON object per line; a whole JSON array of objects is accepted too). Example: `timestamp,level,message` then `2024-06-01T10:00:05Z,ERROR,upstream timeout`. Up to 200000 lines." },
                    "format": { "type": "string", "enum": ["auto", "csv", "tsv", "jsonl"], "default": "auto", "description": "Input format. auto (default) reads the first non-blank line and picks jsonl when it starts with { or [, otherwise delimited text. Set csv/tsv/jsonl to override a bad guess." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "Field separator for CSV/TSV input. auto (default) counts commas, semicolons, tabs and pipes outside quotes on the first line and picks the most common. Ignored for jsonl." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row holds column names. Set false for headerless data — columns are then named column1, column2, … and can be referenced by those names or by 1-based index." },
                    "time_column": { "type": "string", "default": "", "description": "Which column holds the event time: a header name (case-insensitive) or a 1-based index. Empty (default) auto-detects it by header name (timestamp, time, date, created_at, @timestamp, TimeCreated, …) and falls back to the first column whose values actually parse as times." },
                    "from": { "type": "string", "default": "", "description": "Keep only events at or after this time, inclusive — e.g. 2024-06-01, 2024-06-01T10:00:00Z, or an epoch value. A date with no time means 00:00:00. Empty (default) means no lower bound." },
                    "to": { "type": "string", "default": "", "description": "Keep only events at or before this time, inclusive — e.g. 2024-06-02 or 2024-06-02T23:59:59Z. A date with no time covers that whole day through 23:59:59.999. Empty (default) means no upper bound." },
                    "tz_offset": { "type": "number", "default": 0.0, "minimum": -14, "maximum": 14, "description": "Hours that timezone-less timestamps in the data (and in from/to) are offset from UTC, e.g. -5 for US Eastern standard time or 5.5 for India. Values that already carry a Z or ±hh:mm offset are unaffected. Default 0." },
                    "search": { "type": "string", "default": "", "description": "Full-text search: keep rows where any searched column contains this text, e.g. timeout. Case-insensitive substring by default; set regex=true to treat it as a regular expression such as `job \\d+`. Empty (default) searches nothing away." },
                    "search_fields": { "type": "string", "default": "", "description": "Comma-separated columns the search looks in, by header name or 1-based index — e.g. message, service. Empty (default) searches every column." },
                    "regex": { "type": "boolean", "default": false, "description": "When true, `search` is a regular expression instead of a plain substring. Off by default. An invalid pattern is an error naming the syntax problem." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When true, `search` matches case exactly. Off by default, so ERROR finds error. Column filters using contains/startswith/endswith always ignore case." },
                    "filters": { "type": "string", "default": "", "description": "Column conditions, ONE PER LINE, all of which must hold: `<column> <op> <value>` with op one of == != < <= > >= contains !contains startswith endswith matches. Example: `level == ERROR` on one line and `service contains work` on the next. Comparison is numeric when both sides are numbers, otherwise text; `matches` takes a regular expression. Empty (default) keeps every row." },
                    "sort_by": { "type": "string", "default": "", "description": "Column to sort by, as a header name or 1-based index. Empty (default) sorts by the timestamp column; rows whose timestamp does not parse are placed last. Non-time columns sort numerically when both cells are numbers, otherwise alphabetically." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Sort direction: asc (oldest or smallest first, the default) or desc (newest or largest first)." },
                    "columns": { "type": "string", "default": "", "description": "Comma-separated columns to show, in this order, by header name or 1-based index — e.g. timestamp, level, message. Empty (default) shows every column." },
                    "limit": { "type": "integer", "default": 100, "minimum": 1, "maximum": 100000, "description": "How many matching rows to show, i.e. the page size (1-100000, default 100). The match count in the footer always reports the full total, so a trim is never silent." },
                    "offset": { "type": "integer", "default": 0, "minimum": 0, "maximum": 1000000, "description": "How many matching rows to skip before the page starts (default 0). Use with limit to page: offset=100 with limit=100 is the second page." },
                    "output": { "type": "string", "enum": ["table", "csv", "json", "jsonl", "summary"], "default": "table", "description": "Result shape: table (default, an aligned grid with a # source-row-number column and a match-count footer), csv, json (array of objects), jsonl (one object per line), or summary (row counts, detected time column, first/last event and an activity histogram over the whole match set, ignoring limit/offset)." }
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
