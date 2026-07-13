//! gizza-ai/log-analyzer — chat skill block on the shared tool abstraction.
//!
//! Analyzes raw log text and returns an aggregate SUMMARY: counts by level, the
//! most frequent warning/error messages ("top errors"), the time span, and a
//! bucketed volume timeline. Auto-detects JSON/NDJSON, logfmt, syslog, and
//! Apache/nginx access logs. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI + page); `handle()` delegates to
//! `block_utils::run_skill`. No host calls — runs entirely in the WASM sandbox.
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
    /// 0 → the core default (10); the core clamps to 1..=MAX_TOP.
    #[serde(default)]
    top: u32,
    #[serde(default)]
    bucket: String,
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
            Param::enumv("output", ["summary", "json"])
                .default("summary")
                .describe("Output shape. 'summary' (default) is a Markdown report: a caption, a level breakdown, top errors, and a volume timeline; 'json' is a structured object with the same data."),
        )
        .param(
            // Bounds reference the core clamp (MAX_TOP) so the schema can't drift
            // from what `analyze` enforces.
            Param::integer("top")
                .default(10)
                .min(1.0)
                .max(gizza_ai_log_analyzer_core::MAX_TOP as f64)
                .describe("How many top warning/error groups to list (1-100). Similar messages are grouped by masking numbers and hex ids. Default 10."),
        )
        .param(
            Param::enumv("bucket", ["auto", "minute", "hour", "day"])
                .default("auto")
                .describe("Volume-timeline granularity. 'auto' (default) picks minute/hour/day from the time span; or force 'minute', 'hour', or 'day'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LogAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze raw logs: counts by level, top errors, time span, and a volume timeline.",
    skill(
        description = "Analyze raw logs and return an aggregate summary (not a row-by-row view — use log-parser for that). Auto-detects JSON/NDJSON, logfmt, syslog (RFC 3164/5424), and Apache/nginx access logs, then reports: total entries and time span, a breakdown of counts by unified severity (trace/debug/info/warn/error — 5xx and 4xx access-log statuses map to error/warn), the most frequent warning/error messages ('top errors', grouped by masking numbers and hex ids), and a volume timeline bucketed by minute/hour/day. output='summary' (default) is Markdown; 'json' is a structured object. top caps the error list (default 10); bucket sets the timeline granularity (default auto).",
        parameters = schema_json()
    ),
)]
impl LogAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "log-analyzer", |a: Args| {
            gizza_ai_log_analyzer_core::analyze(&a.logs, &a.format, &a.output, a.top, &a.bucket)
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
                    "output": { "type": "string", "enum": ["summary", "json"], "default": "summary", "description": "Output shape. 'summary' (default) is a Markdown report: a caption, a level breakdown, top errors, and a volume timeline; 'json' is a structured object with the same data." },
                    "top": { "type": "integer", "minimum": 1, "maximum": 100, "default": 10, "description": "How many top warning/error groups to list (1-100). Similar messages are grouped by masking numbers and hex ids. Default 10." },
                    "bucket": { "type": "string", "enum": ["auto", "minute", "hour", "day"], "default": "auto", "description": "Volume-timeline granularity. 'auto' (default) picks minute/hour/day from the time span; or force 'minute', 'hour', or 'day'." }
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
