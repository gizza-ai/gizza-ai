//! gizza-ai/email-phishing-link-scanner — scan every link in a pasted email for phishing signals.
//!
//! Thin chat-skill wrapper around `gizza-ai-email-phishing-link-scanner-core`. The chat schema is
//! single-sourced from `descriptor()` (shared across chat, CLI, and page query params); the
//! handler delegates to `block_utils::run_skill`. No host calls — every check is deterministic and
//! offline, so the same message always produces the same report.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
    #[serde(default)]
    brands: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_report")]
    report: String,
    #[serde(default)]
    only_flagged: bool,
    #[serde(default = "default_max_links")]
    max_links: i64,
}

fn default_format() -> String {
    "auto".to_string()
}

fn default_report() -> String {
    "detailed".to_string()
}

fn default_max_links() -> i64 {
    200
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("email")
                .required()
                .describe("The message to scan. Paste either a raw RFC 5322 email (header lines, a blank line, then the body), an HTML email body, or plain text containing links. Every <a href> and every bare http(s) URL is extracted and rated separately. Example: a body containing '<a href=\"http://192.0.2.9/login\">https://www.paypal.com/signin</a>'. Maximum 1048576 bytes."),
        )
        .param(
            Param::string("brands")
                .default("")
                .describe("Extra domains to treat as impersonation targets, in addition to the ~50 built-in ones and the message's own sender domain. Comma-, space-, or newline-separated; a full URL is reduced to its domain. Example: 'acmecorp.com, acme-bank.co.uk'. Defaults to empty (built-in list only)."),
        )
        .param(
            Param::enumv("format", ["auto", "raw", "html", "text"])
                .default("auto")
                .describe("How to read the input: 'auto' (default) detects a header block and HTML markup on its own; 'raw' requires an RFC 5322 header block; 'html' treats the whole input as an HTML body; 'text' treats it as plain text with no headers and no markup."),
        )
        .param(
            Param::enumv("report", ["detailed", "summary", "json"])
                .default("detailed")
                .describe("Output shape: 'detailed' (default) lists every link with its target, its visible text, and each finding; 'summary' prints the overall rating plus one line per flagged link; 'json' returns a machine-readable object with the rating, counts, and per-link findings."),
        )
        .param(
            Param::boolean("only_flagged")
                .default(false)
                .describe("Set true to list only the links that raised at least one finding, hiding clean ones. The counts and the overall rating always cover every scanned link. Defaults to false."),
        )
        .param(
            Param::integer("max_links")
                .default(200)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of links to scan, in the order they appear. Extra links are counted and reported as truncated but not rated. Range 1-1000, default 200."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmailPhishingLinkScanner;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-phishing-link-scanner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan every link in an email for phishing signals",
    skill(
        description = "Scan an email for suspicious links, offline and deterministically. Accepts a raw RFC 5322 message, an HTML body, or plain text, extracts every <a href> and bare http(s) URL, and rates each link 0-100 with a MINIMAL/LOW/MEDIUM/HIGH/CRITICAL band plus the exact findings behind it. Per-link checks cover display-text vs actual-target mismatch, lookalike domains (homoglyph and digit swaps, punycode decoded before comparison, typosquats by edit distance, combosquats, brand names buried in a subdomain, and brand names on a different suffix) against ~50 built-in brands plus your own list plus the message's sender domain, bare-IP hosts, '@' userinfo tricks, redirect wrappers and open redirects (a single level is unwrapped and the destination scanned), link shorteners, abused TLDs, percent-encoded hosts, plain http, non-standard ports, deep subdomains, hyphen-stacked hosts, credential keywords, excessive length, and digit-heavy hosts. Returns the overall rating, how many links were scanned and flagged, and the per-link detail. It never performs DNS, WHOIS, HTTP, blocklist, or reputation lookups, so a MINIMAL rating means no structural red flags, not proof a link is safe.",
        parameters = schema_json()
    ),
)]
impl EmailPhishingLinkScanner {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-phishing-link-scanner", |a: Args| {
            gizza_ai_email_phishing_link_scanner_core::run(
                &a.email,
                &a.brands,
                &a.format,
                &a.report,
                a.only_flagged,
                a.max_links,
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
                    "email": { "type": "string", "description": "The message to scan. Paste either a raw RFC 5322 email (header lines, a blank line, then the body), an HTML email body, or plain text containing links. Every <a href> and every bare http(s) URL is extracted and rated separately. Example: a body containing '<a href=\"http://192.0.2.9/login\">https://www.paypal.com/signin</a>'. Maximum 1048576 bytes." },
                    "brands": { "type": "string", "default": "", "description": "Extra domains to treat as impersonation targets, in addition to the ~50 built-in ones and the message's own sender domain. Comma-, space-, or newline-separated; a full URL is reduced to its domain. Example: 'acmecorp.com, acme-bank.co.uk'. Defaults to empty (built-in list only)." },
                    "format": { "type": "string", "enum": ["auto", "raw", "html", "text"], "default": "auto", "description": "How to read the input: 'auto' (default) detects a header block and HTML markup on its own; 'raw' requires an RFC 5322 header block; 'html' treats the whole input as an HTML body; 'text' treats it as plain text with no headers and no markup." },
                    "report": { "type": "string", "enum": ["detailed", "summary", "json"], "default": "detailed", "description": "Output shape: 'detailed' (default) lists every link with its target, its visible text, and each finding; 'summary' prints the overall rating plus one line per flagged link; 'json' returns a machine-readable object with the rating, counts, and per-link findings." },
                    "only_flagged": { "type": "boolean", "default": false, "description": "Set true to list only the links that raised at least one finding, hiding clean ones. The counts and the overall rating always cover every scanned link. Defaults to false." },
                    "max_links": { "type": "integer", "default": 200, "minimum": 1, "maximum": 1000, "description": "Maximum number of links to scan, in the order they appear. Extra links are counted and reported as truncated but not rated. Range 1-1000, default 200." }
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
