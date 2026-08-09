//! gizza-ai/email-spam-score — transparent, rule-by-rule spam scoring for an email.
//!
//! Thin chat-skill wrapper around `gizza-ai-email-spam-score-core`. The chat schema is
//! single-sourced from `descriptor()` (shared across chat, CLI, and page query params); the
//! handler delegates to `block_utils::run_skill`. No host calls — every rule is deterministic and
//! offline, so the same message always produces the same score.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
    #[serde(default)]
    subject: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_report")]
    report: String,
    #[serde(default = "default_check_headers")]
    check_headers: bool,
}

fn default_format() -> String {
    "auto".to_string()
}

fn default_report() -> String {
    "detailed".to_string()
}

fn default_check_headers() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("email")
                .required()
                .describe("The message to score. Paste either a raw RFC 5322 email (header lines, a blank line, then the body), an HTML email body, or plain body text. Example: a message whose header block starts with 'From: Alerts <notice@example.com>' and whose body reads 'ACT NOW to claim your FREE GIFT!'. Maximum 1048576 bytes."),
        )
        .param(
            Param::string("subject")
                .default("")
                .describe("Subject line to score when the pasted input has no 'Subject:' header. Ignored if a Subject header is present. Example: 'Your invoice for March'. Defaults to empty."),
        )
        .param(
            Param::enumv("format", ["auto", "raw", "html", "text"])
                .default("auto")
                .describe("How to read the input: 'auto' (default) detects a header block and HTML markup on its own; 'raw' requires an RFC 5322 header block; 'html' treats the whole input as an HTML body; 'text' treats it as plain body text with no headers and no markup."),
        )
        .param(
            Param::enumv("report", ["detailed", "summary", "json"])
                .default("detailed")
                .describe("Output shape: 'detailed' (default) prints the score, message stats, and every rule that fired with its points; 'summary' prints the score, verdict, and top three signals; 'json' returns a machine-readable object with score, band, stats, and the rule list."),
        )
        .param(
            Param::boolean("check_headers")
                .default(true)
                .describe("Whether to apply the header-anomaly rules (SPF/DKIM/DMARC results, From vs Return-Path, Reply-To detours, display-name spoofing, Message-ID, Date, Received, recipient hiding). Set false to score body content only. Has no effect when the input contains no header block. Defaults to true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmailSpamScore;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-spam-score",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Score an email's spamminess with transparent, weighted heuristics",
    skill(
        description = "Score how spammy an email looks using transparent, deterministic heuristics — no model, no network, no account. Accepts a raw RFC 5322 message, an HTML body, or plain body text and returns a 0-100 score (higher is spammier) with a LOW/MEDIUM/HIGH/CRITICAL band, message stats (words, uppercase ratio, links, unique link domains, link density, images, trigger-phrase hits, punctuation runs), and every rule that fired with its point contribution. Content rules cover weighted spam trigger phrases across six categories, ALL-CAPS shouting, repeated punctuation and character runs, link count and link density, URL shorteners, suspicious TLDs, insecure http links, obfuscated URLs (userinfo, punycode, bare IP), anchor-text vs href mismatch, image-to-text ratio, tracking pixels, hidden text, zero-width characters, homoglyph mixing, multiple embedded addresses, length extremes, and large headline money amounts. Header rules read the Authentication-Results the receiving gateway already stamped (SPF/DKIM/DMARC) plus From vs Return-Path, Reply-To detours, display-name spoofing, Message-ID, Date, Received and recipient hiding; passing authentication and a working unsubscribe reduce the score. It never performs DNS, SMTP, HTTP, blacklist, or reputation lookups, so it is a reproducible pre-send or triage signal, not a deliverability verdict or a SpamAssassin result.",
        parameters = schema_json()
    ),
)]
impl EmailSpamScore {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-spam-score", |a: Args| {
            gizza_ai_email_spam_score_core::run(
                &a.email,
                &a.subject,
                &a.format,
                &a.report,
                a.check_headers,
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
                    "email": { "type": "string", "description": "The message to score. Paste either a raw RFC 5322 email (header lines, a blank line, then the body), an HTML email body, or plain body text. Example: a message whose header block starts with 'From: Alerts <notice@example.com>' and whose body reads 'ACT NOW to claim your FREE GIFT!'. Maximum 1048576 bytes." },
                    "subject": { "type": "string", "default": "", "description": "Subject line to score when the pasted input has no 'Subject:' header. Ignored if a Subject header is present. Example: 'Your invoice for March'. Defaults to empty." },
                    "format": { "type": "string", "enum": ["auto", "raw", "html", "text"], "default": "auto", "description": "How to read the input: 'auto' (default) detects a header block and HTML markup on its own; 'raw' requires an RFC 5322 header block; 'html' treats the whole input as an HTML body; 'text' treats it as plain body text with no headers and no markup." },
                    "report": { "type": "string", "enum": ["detailed", "summary", "json"], "default": "detailed", "description": "Output shape: 'detailed' (default) prints the score, message stats, and every rule that fired with its points; 'summary' prints the score, verdict, and top three signals; 'json' returns a machine-readable object with score, band, stats, and the rule list." },
                    "check_headers": { "type": "boolean", "default": true, "description": "Whether to apply the header-anomaly rules (SPF/DKIM/DMARC results, From vs Return-Path, Reply-To detours, display-name spoofing, Message-ID, Date, Received, recipient hiding). Set false to score body content only. Has no effect when the input contains no header block. Defaults to true." }
                },
                "required": ["email"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
