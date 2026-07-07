//! gizza-ai/log-parser — chat skill block on the shared tool abstraction.
//!
//! Auto-detects the log format (JSON/NDJSON, logfmt, syslog, Apache/nginx
//! common + combined access logs) of raw log text and parses it into a
//! structured, filterable view (Markdown table, JSON array, or CSV). The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI +
//! page); `handle()` delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    level: String,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    regex: bool,
    /// 0 → the core default (200); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .describe("The raw log text — one entry per line. Paste JSON/NDJSON, logfmt, syslog, or Apache/nginx access logs."),
        )
        .param(
            Param::enumv("format", ["auto", "json", "logfmt", "syslog", "common", "combined"])
                .default("auto")
                .describe("Log format. 'auto' (default) detects it by majority vote over the first lines; or force 'json' (JSON/NDJSON), 'logfmt' (key=value pairs), 'syslog' (RFC 3164/5424), 'common' (Apache/nginx Common Log Format), or 'combined' (Common + referer + user-agent)."),
        )
        .param(
            Param::enumv("output", ["table", "json", "csv"])
                .default("table")
                .describe("Output shape. 'table' (default) is a Markdown table with a stats caption; 'json' is an array of one object per line; 'csv' is header + rows."),
        )
        .param(
            Param::enumv("level", ["all", "error", "warn", "info", "debug", "trace"])
                .default("all")
                .describe("Minimum severity to keep. 'all' (default) keeps every line; 'warn' keeps warnings and errors, etc. Severity is unified across formats: a JSON/logfmt level key, a syslog priority, or an HTTP status (5xx=error, 4xx=warn)."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Keep only lines that match this text. Case-insensitive substring by default; set regex=true to match a regular expression against the raw line. Blank = no filter."),
        )
        .param(
            Param::boolean("regex")
                .default(false)
                .describe("When true, treat 'filter' as a regular expression instead of a plain substring. Default false."),
        )
        .param(
            // Bounds reference the core clamp (MAX_LIMIT) so the schema can't
            // drift from what `parse` actually enforces.
            Param::integer("limit")
                .default(200)
                .min(1.0)
                .max(gizza_ai_log_parser_core::MAX_LIMIT as f64)
                .describe("Maximum number of rows to output (1-5000). Applied after the level and text filters. Default 200."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LogParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-detect and parse JSON, logfmt, syslog, and access logs into a filterable table.",
    skill(
        description = "Auto-detect and parse raw logs into a structured, filterable table. Handles JSON/NDJSON, logfmt (key=value), syslog (RFC 3164/5424), and Apache/nginx access logs (common + combined). format='auto' (default) detects the format; output='table' (default) renders a Markdown table with a stats caption, or 'json'/'csv'. Filter with level (a unified minimum severity across formats — 5xx and 4xx access-log statuses map to error/warn) and filter (case-insensitive substring, or a regex when regex=true). limit caps the row count (default 200).",
        parameters = schema_json()
    ),
)]
impl LogParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "log-parser", |a: Args| {
            gizza_ai_log_parser_core::parse(
                &a.logs, &a.format, &a.output, &a.level, &a.filter, a.regex, a.limit,
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
                    "logs": { "type": "string", "description": "The raw log text — one entry per line. Paste JSON/NDJSON, logfmt, syslog, or Apache/nginx access logs." },
                    "format": { "type": "string", "enum": ["auto", "json", "logfmt", "syslog", "common", "combined"], "default": "auto", "description": "Log format. 'auto' (default) detects it by majority vote over the first lines; or force 'json' (JSON/NDJSON), 'logfmt' (key=value pairs), 'syslog' (RFC 3164/5424), 'common' (Apache/nginx Common Log Format), or 'combined' (Common + referer + user-agent)." },
                    "output": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output shape. 'table' (default) is a Markdown table with a stats caption; 'json' is an array of one object per line; 'csv' is header + rows." },
                    "level": { "type": "string", "enum": ["all", "error", "warn", "info", "debug", "trace"], "default": "all", "description": "Minimum severity to keep. 'all' (default) keeps every line; 'warn' keeps warnings and errors, etc. Severity is unified across formats: a JSON/logfmt level key, a syslog priority, or an HTTP status (5xx=error, 4xx=warn)." },
                    "filter": { "type": "string", "default": "", "description": "Keep only lines that match this text. Case-insensitive substring by default; set regex=true to match a regular expression against the raw line. Blank = no filter." },
                    "regex": { "type": "boolean", "default": false, "description": "When true, treat 'filter' as a regular expression instead of a plain substring. Default false." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 200, "description": "Maximum number of rows to output (1-5000). Applied after the level and text filters. Default 200." }
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
