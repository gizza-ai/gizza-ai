//! gizza-ai/phishing-header-inspector — inspect raw email headers for spoofing signals.
//!
//! Thin chat-skill wrapper around `gizza-ai-phishing-header-inspector-core`. The chat
//! schema is single-sourced from `descriptor()` (shared across chat, CLI, and page
//! query params); the handler delegates to `block_utils::run_skill`. No host calls —
//! all checks are deterministic and offline.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    headers: String,
    #[serde(default = "default_report_mode")]
    report_mode: String,
    #[serde(default = "default_check_received")]
    check_received: bool,
}

fn default_report_mode() -> String {
    "detailed".to_string()
}

fn default_check_received() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("headers")
                .required()
                .describe("Raw RFC 5322 email headers to inspect. Paste the header block including fields such as From, Return-Path, Authentication-Results, Received, Reply-To, DKIM-Signature, and Message-ID. The message body is ignored after the first blank line."),
        )
        .param(
            Param::enumv("report_mode", ["detailed", "summary"])
                .default("detailed")
                .describe("How much text to return: 'detailed' includes every finding and recommended checks; 'summary' returns identity/authentication fields plus the top findings."),
        )
        .param(
            Param::boolean("check_received")
                .default(true)
                .describe("Whether to inspect the Received header chain for missing/short relay paths and private/internal IP references. Disable for copied header snippets that intentionally omit Received lines."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PhishingHeaderInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/phishing-header-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect email headers for spoofing and phishing indicators",
    skill(
        description = "Inspect pasted raw email headers for offline phishing and spoofing indicators. The tool compares From, Return-Path, Reply-To, display-name, Message-ID, SPF, DKIM, DMARC, and Received-hop evidence; highlights mismatched sender domains, failed or missing authentication results, suspicious reply paths, short or missing Received chains, and private/internal relay references; and returns a deterministic MINIMAL/LOW/MEDIUM/HIGH/CRITICAL risk score. It never performs DNS, reputation, HTTP, SMTP, or mailbox lookups, so it is safe for private browser-local triage but not a final deliverability verdict.",
        parameters = schema_json()
    ),
)]
impl PhishingHeaderInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "phishing-header-inspector", |a: Args| {
            gizza_ai_phishing_header_inspector_core::run(
                &a.headers,
                &a.report_mode,
                a.check_received,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored schema,
    /// so any future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "headers": { "type": "string", "description": "Raw RFC 5322 email headers to inspect. Paste the header block including fields such as From, Return-Path, Authentication-Results, Received, Reply-To, DKIM-Signature, and Message-ID. The message body is ignored after the first blank line." },
                    "report_mode": { "type": "string", "enum": ["detailed", "summary"], "default": "detailed", "description": "How much text to return: 'detailed' includes every finding and recommended checks; 'summary' returns identity/authentication fields plus the top findings." },
                    "check_received": { "type": "boolean", "default": true, "description": "Whether to inspect the Received header chain for missing/short relay paths and private/internal IP references. Disable for copied header snippets that intentionally omit Received lines." }
                },
                "required": ["headers"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
