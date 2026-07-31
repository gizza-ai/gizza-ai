//! gizza-ai/syslog-triage — chat skill block on the shared tool abstraction.
//!
//! Parses Linux syslog / auth.log text into structured security events,
//! classifies each into sudo / ssh / cron / session / account / other with a
//! derived success/failure status, and renders an intrusion-review summary, a
//! Markdown table, or a JSON array. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI + page); `handle()` delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    only: String,
    #[serde(default)]
    output: String,
    /// 0 → the core default (500); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .describe("The raw syslog / auth.log text — one entry per line. Handles BSD syslog (RFC 3164, 'Mmm dd HH:MM:SS host tag[pid]: msg') and rsyslog ISO/RFC 3339 timestamps, with an optional <PRI> prefix."),
        )
        .param(
            Param::enumv("category", ["all", "sudo", "ssh", "cron", "session", "account", "other"])
                .default("all")
                .describe("Keep only events in this category. 'all' (default) keeps every line; 'sudo' = privilege escalation, 'ssh' = sshd auth (accepted/failed/invalid user), 'cron' = cron jobs, 'session' = su/login/logind sessions, 'account' = user/group management, 'other' = unclassified. Category is derived from the service tag."),
        )
        .param(
            Param::enumv("only", ["all", "failed", "success"])
                .default("all")
                .describe("Status filter. 'all' (default) keeps every event; 'failed' keeps only failures (failed passwords, invalid users, sudo/auth denials); 'success' keeps only successful logins, sudo commands, and opened sessions."),
        )
        .param(
            Param::enumv("output", ["summary", "table", "json"])
                .default("summary")
                .describe("Output shape. 'summary' (default) is an intrusion-review report: an events/failed header, category counts, failed logins ranked by source IP, plus sudo and cron sections. 'table' is a Markdown table (one row per event); 'json' is an array of event objects."),
        )
        .param(
            // Bounds reference the core clamp (MAX_LIMIT) so the schema can't
            // drift from what `triage` actually enforces.
            Param::integer("limit")
                .default(500)
                .min(1.0)
                .max(gizza_ai_syslog_triage_core::MAX_LIMIT as f64)
                .describe("Maximum number of events to keep (1-5000). Applied after the category and status filters. Default 500."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SyslogTriage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/syslog-triage",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse and filter Linux syslog/auth.log, highlighting sudo, SSH auth, and cron events for intrusion review.",
    skill(
        description = "Parse and triage Linux syslog / auth.log text for a quick intrusion review. Classifies each line into a security category — sudo, ssh, cron, session, account, or other — derives a success/failure status, and extracts the user, source IP, and command. category filters to one category (default all); only filters by status (all/failed/success — 'failed' surfaces brute-force attempts); output='summary' (default) is an intrusion-review report that ranks failed logins by source IP and lists sudo and cron activity, or 'table'/'json'. limit caps the event count (default 500).",
        parameters = schema_json()
    ),
)]
impl SyslogTriage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "syslog-triage", |a: Args| {
            gizza_ai_syslog_triage_core::triage(
                &a.logs, &a.category, &a.only, &a.output, a.limit,
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
                    "logs": { "type": "string", "description": "The raw syslog / auth.log text — one entry per line. Handles BSD syslog (RFC 3164, 'Mmm dd HH:MM:SS host tag[pid]: msg') and rsyslog ISO/RFC 3339 timestamps, with an optional <PRI> prefix." },
                    "category": { "type": "string", "enum": ["all", "sudo", "ssh", "cron", "session", "account", "other"], "default": "all", "description": "Keep only events in this category. 'all' (default) keeps every line; 'sudo' = privilege escalation, 'ssh' = sshd auth (accepted/failed/invalid user), 'cron' = cron jobs, 'session' = su/login/logind sessions, 'account' = user/group management, 'other' = unclassified. Category is derived from the service tag." },
                    "only": { "type": "string", "enum": ["all", "failed", "success"], "default": "all", "description": "Status filter. 'all' (default) keeps every event; 'failed' keeps only failures (failed passwords, invalid users, sudo/auth denials); 'success' keeps only successful logins, sudo commands, and opened sessions." },
                    "output": { "type": "string", "enum": ["summary", "table", "json"], "default": "summary", "description": "Output shape. 'summary' (default) is an intrusion-review report: an events/failed header, category counts, failed logins ranked by source IP, plus sudo and cron sections. 'table' is a Markdown table (one row per event); 'json' is an array of event objects." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 500, "description": "Maximum number of events to keep (1-5000). Applied after the category and status filters. Default 500." }
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
