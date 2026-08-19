//! gizza-ai/json-log-formatter — chat skill block on the shared tool abstraction.
//!
//! Renders NDJSON / JSON Lines application logs as a readable, aligned log view
//! (or a Markdown table / JSON array / CSV), with dotted flattening of nested
//! objects, a minimum-severity filter that understands level words and numbers,
//! configurable time/level/message key names, and a per-field contains/exact
//! filter. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    level: String,
    #[serde(default)]
    field: String,
    #[serde(default)]
    filter: String,
    #[serde(default, rename = "match")]
    match_mode: String,
    #[serde(default)]
    fields: String,
    #[serde(default)]
    level_field: String,
    #[serde(default)]
    time_field: String,
    #[serde(default)]
    message_field: String,
    #[serde(default = "default_true")]
    flatten: bool,
    #[serde(default)]
    on_invalid: String,
    /// 0 → the core default (200); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The NDJSON / JSON Lines log text — one JSON object per line, e.g. {\"time\":\"2024-01-01T00:00:00Z\",\"level\":\"info\",\"msg\":\"server started\"}. Blank lines and lines starting with # or // are skipped."),
        )
        .param(
            Param::enumv("level", ["all", "trace", "debug", "info", "warn", "error", "fatal"])
                .default("all")
                .describe("Minimum severity to keep. 'all' (default) keeps every record; 'warn' keeps warnings, errors and fatals, etc. Level values may be words (info, warning, crit) or numbers — 10/20/30/40/50/60 bunyan-style, or a syslog priority 7/6/5/4/3/2."),
        )
        .param(
            Param::string("field")
                .default("")
                .describe("Dotted path the 'filter' text is matched against, e.g. req.method or items.0.id (numeric segments index arrays). Blank (default) searches the whole record, nested values included."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Text to keep records by. Blank (default) keeps every record. See 'field' for where it is matched and 'match' for how."),
        )
        .param(
            Param::enumv("match", ["contains", "exact"])
                .default("contains")
                .describe("How 'filter' is compared. 'contains' (default) is a case-insensitive substring test; 'exact' requires the field's stringified value to equal the filter exactly (case-sensitive) — with a blank 'field', 'exact' matches when any single field equals it."),
        )
        .param(
            Param::string("fields")
                .default("")
                .describe("Comma-separated dotted paths to keep, in this order, e.g. 'time, level, msg, req.url'. Missing paths render empty. Blank (default) keeps every key."),
        )
        .param(
            Param::string("level_field")
                .default("")
                .describe("Key holding the severity. Blank (default) auto-detects level, lvl, severity, levelname, loglevel, log_level, @level."),
        )
        .param(
            Param::string("time_field")
                .default("")
                .describe("Key holding the timestamp. Blank (default) auto-detects time, ts, timestamp, @timestamp, datetime, date, t. Numeric values are read as epoch seconds (or milliseconds when too large for seconds) and rendered as ISO 8601 UTC."),
        )
        .param(
            Param::string("message_field")
                .default("")
                .describe("Key holding the log message. Blank (default) auto-detects message, msg, text, body, event, log."),
        )
        .param(
            Param::boolean("flatten")
                .default(true)
                .describe("When true (default), nested objects and arrays expand into dotted keys — req.method, items.0.id. When false, a nested value stays whole and renders as compact JSON."),
        )
        .param(
            Param::enumv("on_invalid", ["skip", "keep", "error"])
                .default("skip")
                .describe("What to do with a line that is not a JSON object. 'skip' (default) drops it and reports the count; 'keep' passes the raw line through as the message; 'error' fails and names the line number."),
        )
        .param(
            // Bounds reference the core clamp (MAX_LIMIT) so the schema can't
            // drift from what `format_logs` actually enforces.
            Param::integer("limit")
                .default(200)
                .min(1.0)
                .max(gizza_ai_json_log_formatter_core::MAX_LIMIT as f64)
                .describe("Maximum number of records to render (1-5000). Applied after the level and text filters. Default 200."),
        )
        .param(
            Param::enumv("output", ["pretty", "table", "json", "csv"])
                .default("pretty")
                .describe("Output shape. 'pretty' (default) is an aligned log view — [time] LEVEL message key=value …; 'table' is a Markdown table; 'json' is a pretty-printed array of one object per record; 'csv' is header + rows."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonLogFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-log-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn NDJSON application logs into an aligned, filterable log view, table, JSON, or CSV.",
    skill(
        description = "Format NDJSON / JSON Lines logs (one JSON object per line) into a readable view. output='pretty' (default) renders aligned '[time] LEVEL message key=value …' lines, or use 'table' (Markdown), 'json', or 'csv'. Nested objects flatten into dotted keys (req.method, items.0.id) unless flatten=false. The time/level/message keys are auto-detected across the usual aliases, or named with time_field/level_field/message_field. level sets a minimum severity and understands level words plus bunyan (10-60) and syslog (0-7) numbers. Filter records with filter + field (a dotted path; blank searches the whole record) and match=contains|exact, project columns with fields, cap rows with limit (default 200), and choose how bad lines are handled with on_invalid=skip|keep|error.",
        parameters = schema_json()
    ),
)]
impl JsonLogFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "json-log-formatter", |a: Args| {
            gizza_ai_json_log_formatter_core::format_logs(
                &a.input,
                &a.level,
                &a.field,
                &a.filter,
                &a.match_mode,
                &a.fields,
                &a.level_field,
                &a.time_field,
                &a.message_field,
                a.flatten,
                &a.on_invalid,
                a.limit,
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
                    "input": { "type": "string", "description": "The NDJSON / JSON Lines log text — one JSON object per line, e.g. {\"time\":\"2024-01-01T00:00:00Z\",\"level\":\"info\",\"msg\":\"server started\"}. Blank lines and lines starting with # or // are skipped." },
                    "level": { "type": "string", "enum": ["all", "trace", "debug", "info", "warn", "error", "fatal"], "default": "all", "description": "Minimum severity to keep. 'all' (default) keeps every record; 'warn' keeps warnings, errors and fatals, etc. Level values may be words (info, warning, crit) or numbers — 10/20/30/40/50/60 bunyan-style, or a syslog priority 7/6/5/4/3/2." },
                    "field": { "type": "string", "default": "", "description": "Dotted path the 'filter' text is matched against, e.g. req.method or items.0.id (numeric segments index arrays). Blank (default) searches the whole record, nested values included." },
                    "filter": { "type": "string", "default": "", "description": "Text to keep records by. Blank (default) keeps every record. See 'field' for where it is matched and 'match' for how." },
                    "match": { "type": "string", "enum": ["contains", "exact"], "default": "contains", "description": "How 'filter' is compared. 'contains' (default) is a case-insensitive substring test; 'exact' requires the field's stringified value to equal the filter exactly (case-sensitive) — with a blank 'field', 'exact' matches when any single field equals it." },
                    "fields": { "type": "string", "default": "", "description": "Comma-separated dotted paths to keep, in this order, e.g. 'time, level, msg, req.url'. Missing paths render empty. Blank (default) keeps every key." },
                    "level_field": { "type": "string", "default": "", "description": "Key holding the severity. Blank (default) auto-detects level, lvl, severity, levelname, loglevel, log_level, @level." },
                    "time_field": { "type": "string", "default": "", "description": "Key holding the timestamp. Blank (default) auto-detects time, ts, timestamp, @timestamp, datetime, date, t. Numeric values are read as epoch seconds (or milliseconds when too large for seconds) and rendered as ISO 8601 UTC." },
                    "message_field": { "type": "string", "default": "", "description": "Key holding the log message. Blank (default) auto-detects message, msg, text, body, event, log." },
                    "flatten": { "type": "boolean", "default": true, "description": "When true (default), nested objects and arrays expand into dotted keys — req.method, items.0.id. When false, a nested value stays whole and renders as compact JSON." },
                    "on_invalid": { "type": "string", "enum": ["skip", "keep", "error"], "default": "skip", "description": "What to do with a line that is not a JSON object. 'skip' (default) drops it and reports the count; 'keep' passes the raw line through as the message; 'error' fails and names the line number." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 200, "description": "Maximum number of records to render (1-5000). Applied after the level and text filters. Default 200." },
                    "output": { "type": "string", "enum": ["pretty", "table", "json", "csv"], "default": "pretty", "description": "Output shape. 'pretty' (default) is an aligned log view — [time] LEVEL message key=value …; 'table' is a Markdown table; 'json' is a pretty-printed array of one object per record; 'csv' is header + rows." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page passes fields positionally to the wasm export, so the page form's
    /// [[input]] order must stay in lockstep with the descriptor's param order.
    #[test]
    fn descriptor_param_order_matches_the_page_form() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let names: Vec<&str> = derived["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "input",
                "level",
                "field",
                "filter",
                "match",
                "fields",
                "level_field",
                "time_field",
                "message_field",
                "flatten",
                "on_invalid",
                "limit",
                "output"
            ]
        );
    }
}
