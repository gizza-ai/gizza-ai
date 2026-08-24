//! gizza-ai/weblog-attack-analyzer — flag suspicious Apache/Nginx/IIS access-log requests.
//!
//! Pure Rust: pasted log text is parsed locally, request targets/user agents are
//! matched against web-attack signatures, and per-IP offender behaviour is
//! aggregated. The chat schema is single-sourced from descriptor() (CLI + page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_category() -> String {
    "all".into()
}
fn default_min_severity() -> String {
    "all".into()
}
fn default_output() -> String {
    "report".into()
}
fn default_decode() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default = "default_category")]
    category: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    offender_threshold: u32,
    #[serde(default)]
    error_threshold: u32,
    #[serde(default = "default_decode")]
    decode: bool,
    #[serde(default)]
    limit: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .multiline()
                .describe("Apache/Nginx common or combined access-log lines, or IIS W3C extended logs with a #Fields header. Paste one request per line."),
        )
        .param(
            Param::enumv("category", ["all", "sqli", "xss", "traversal", "rce", "file-include", "scanner", "probe"])
                .default("all")
                .describe("Attack category filter: all (default), SQL injection, XSS, path traversal, remote code execution, file inclusion, scanner user agents, or sensitive-path probes."),
        )
        .param(
            Param::enumv("min_severity", ["all", "low", "medium", "high", "critical"])
                .default("all")
                .describe("Minimum severity to report. all (default) includes low through critical; high hides low/medium probes; critical shows only critical SQLi/RCE-style findings."),
        )
        .param(
            Param::enumv("output", ["report", "table", "json", "csv", "blocklist"])
                .default("report")
                .describe("Output format: report (default Markdown-style summary), table, json, csv, or blocklist (one offending IP per line)."),
        )
        .param(
            Param::integer("offender_threshold")
                .default(20)
                .min(1.0)
                .max(gizza_ai_weblog_attack_analyzer_core::MAX_THRESHOLD as f64)
                .describe("Requests from one source IP needed to label it high-volume (1-100000). Default 20; pass 0 in CLI/chat to use the default."),
        )
        .param(
            Param::integer("error_threshold")
                .default(5)
                .min(1.0)
                .max(gizza_ai_weblog_attack_analyzer_core::MAX_THRESHOLD as f64)
                .describe("404s or 401/403s from one source IP needed to label enumeration or brute-force behaviour (1-100000). Default 5; pass 0 to use the default."),
        )
        .param(
            Param::boolean("decode")
                .default(true)
                .describe("Also scan once- and twice-percent-decoded request targets so encoded payloads are caught. Default true."),
        )
        .param(
            Param::integer("limit")
                .default(500)
                .min(1.0)
                .max(gizza_ai_weblog_attack_analyzer_core::MAX_LIMIT as f64)
                .describe("Maximum flagged requests to render in report/table/json/csv outputs (1-5000). Default 500; counts always cover the whole log."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/weblog-attack-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag web attack attempts and noisy offenders in access logs",
    skill(
        description = "Analyze pasted Apache/Nginx or IIS access logs for common web-attack traffic. Flags SQL injection, XSS, path traversal, file inclusion, RCE payloads, scanner user agents (sqlmap, nikto, nuclei, etc.), sensitive-path probes (.env, .git, wp-login, phpMyAdmin), and high-volume / enumeration / brute-force source IPs. Percent-decodes request targets by default to catch encoded payloads. Outputs a report, table, JSON, CSV, or an IP blocklist. Runs locally on pasted log text; it is a heuristic triage aid, not a WAF or IDS replacement.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "weblog-attack-analyzer", |a: Args| {
            gizza_ai_weblog_attack_analyzer_core::analyze(
                &a.logs,
                &a.category,
                &a.min_severity,
                &a.output,
                a.offender_threshold,
                a.error_threshold,
                a.decode,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "logs": { "type": "string", "description": "Apache/Nginx common or combined access-log lines, or IIS W3C extended logs with a #Fields header. Paste one request per line." },
                    "category": { "type": "string", "enum": ["all","sqli","xss","traversal","rce","file-include","scanner","probe"], "default": "all", "description": "Attack category filter: all (default), SQL injection, XSS, path traversal, remote code execution, file inclusion, scanner user agents, or sensitive-path probes." },
                    "min_severity": { "type": "string", "enum": ["all","low","medium","high","critical"], "default": "all", "description": "Minimum severity to report. all (default) includes low through critical; high hides low/medium probes; critical shows only critical SQLi/RCE-style findings." },
                    "output": { "type": "string", "enum": ["report","table","json","csv","blocklist"], "default": "report", "description": "Output format: report (default Markdown-style summary), table, json, csv, or blocklist (one offending IP per line)." },
                    "offender_threshold": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 20, "description": "Requests from one source IP needed to label it high-volume (1-100000). Default 20; pass 0 in CLI/chat to use the default." },
                    "error_threshold": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 5, "description": "404s or 401/403s from one source IP needed to label enumeration or brute-force behaviour (1-100000). Default 5; pass 0 to use the default." },
                    "decode": { "type": "boolean", "default": true, "description": "Also scan once- and twice-percent-decoded request targets so encoded payloads are caught. Default true." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 500, "description": "Maximum flagged requests to render in report/table/json/csv outputs (1-5000). Default 500; counts always cover the whole log." }
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
