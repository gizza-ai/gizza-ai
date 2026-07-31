//! gizza-ai/log-to-table — chat skill block on the shared tool abstraction.
//!
//! Parses semi-structured log lines into a table / CSV / TSV / JSON using a
//! regex template whose named capture groups `(?P<name>...)` define the columns,
//! with presets for common formats (Apache common & combined access logs,
//! syslog, log4j). The chat schema is single-sourced from `descriptor()` (which
//! also drives the CLI + page); `handle()` delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default)]
    preset: String,
    #[serde(default)]
    pattern: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    on_nomatch: String,
    /// 0 → the core default (500); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .describe("The raw log text — one entry per line. Blank lines are ignored."),
        )
        .param(
            Param::enumv("preset", ["custom", "common", "combined", "syslog", "log4j"])
                .default("custom")
                .describe("Format preset supplying a ready-made regex. 'custom' (default) uses your own 'pattern'; 'common' = Apache/nginx Common Log Format; 'combined' = Combined (common + referer + user-agent); 'syslog' = RFC 3164; 'log4j' = 'YYYY-MM-DD HH:MM:SS LEVEL logger - message'."),
        )
        .param(
            Param::string("pattern")
                .default("")
                .describe("A regular expression whose named capture groups (?P<name>...) become the table columns, in order. Required when preset='custom'; ignored otherwise. Example: ^(?P<ip>\\S+) (?P<status>\\d{3}) (?P<path>\\S+)$. No backreferences/lookaround (linear-time engine)."),
        )
        .param(
            Param::enumv("output", ["table", "csv", "tsv", "json"])
                .default("table")
                .describe("Output shape. 'table' (default) is an aligned Markdown table with a row-count caption; 'csv' and 'tsv' are RFC-4180-quoted delimited text; 'json' is an array of one object per line."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Include a header row of column names (table/csv/tsv). Default true. JSON always keys by column name."),
        )
        .param(
            Param::enumv("on_nomatch", ["skip", "keep", "error"])
                .default("skip")
                .describe("What to do with a line that does not match the pattern. 'skip' (default) drops it (counted in the caption); 'keep' emits a row with the raw line in an 'unparsed' column; 'error' fails on the first non-matching line."),
        )
        .param(
            // Bounds reference the core clamp (MAX_LIMIT) so the schema can't
            // drift from what `parse` actually enforces.
            Param::integer("limit")
                .default(500)
                .min(1.0)
                .max(gizza_ai_log_to_table_core::MAX_LIMIT as f64)
                .describe("Maximum number of rows to output (1-5000). Default 500."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LogToTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-to-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse log lines into a table/CSV with a regex template and presets for common formats.",
    skill(
        description = "Parse semi-structured log lines into a table, CSV, TSV, or JSON using a regex template whose named capture groups (?P<name>...) define the columns. Pick a preset (common/combined Apache access logs, syslog, log4j) for a ready-made regex, or preset='custom' with your own 'pattern'. output='table' (default) renders an aligned Markdown table; header toggles the header row; on_nomatch handles lines that don't match (skip/keep/error); limit caps rows (default 500).",
        parameters = schema_json()
    ),
)]
impl LogToTable {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "log-to-table", |a: Args| {
            gizza_ai_log_to_table_core::parse(
                &a.logs,
                &a.preset,
                &a.pattern,
                &a.output,
                a.header,
                &a.on_nomatch,
                a.limit,
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
                    "logs": { "type": "string", "description": "The raw log text — one entry per line. Blank lines are ignored." },
                    "preset": { "type": "string", "enum": ["custom", "common", "combined", "syslog", "log4j"], "default": "custom", "description": "Format preset supplying a ready-made regex. 'custom' (default) uses your own 'pattern'; 'common' = Apache/nginx Common Log Format; 'combined' = Combined (common + referer + user-agent); 'syslog' = RFC 3164; 'log4j' = 'YYYY-MM-DD HH:MM:SS LEVEL logger - message'." },
                    "pattern": { "type": "string", "default": "", "description": "A regular expression whose named capture groups (?P<name>...) become the table columns, in order. Required when preset='custom'; ignored otherwise. Example: ^(?P<ip>\\S+) (?P<status>\\d{3}) (?P<path>\\S+)$. No backreferences/lookaround (linear-time engine)." },
                    "output": { "type": "string", "enum": ["table", "csv", "tsv", "json"], "default": "table", "description": "Output shape. 'table' (default) is an aligned Markdown table with a row-count caption; 'csv' and 'tsv' are RFC-4180-quoted delimited text; 'json' is an array of one object per line." },
                    "header": { "type": "boolean", "default": true, "description": "Include a header row of column names (table/csv/tsv). Default true. JSON always keys by column name." },
                    "on_nomatch": { "type": "string", "enum": ["skip", "keep", "error"], "default": "skip", "description": "What to do with a line that does not match the pattern. 'skip' (default) drops it (counted in the caption); 'keep' emits a row with the raw line in an 'unparsed' column; 'error' fails on the first non-matching line." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 500, "description": "Maximum number of rows to output (1-5000). Default 500." }
                },
                "required": ["logs"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
