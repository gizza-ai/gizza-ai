//! gizza-ai/url-safety-inspect — rate a single URL's phishing risk from purely
//! structural heuristics (no network, no blocklists).
//!
//! Thin chat-skill wrapper around `gizza-ai-url-safety-inspect-core`. The chat schema is
//! derived from `descriptor()` (single source — shared across chat + CLI + page
//! query-params); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_safety_inspect_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("url")
            .required()
            .describe("The URL to inspect (e.g. https://secure-login.example.tk/verify). A scheme is optional; whitespace is trimmed. Only the URL's structure is examined — no request is made to the site."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UrlSafetyInspect;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-safety-inspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rate a URL's phishing risk from structural heuristics",
    skill(
        description = "Apply offline structural heuristics to a single URL and rate its phishing risk, without contacting the site or any blocklist. Checks for an IP-literal host, an '@' in the authority (userinfo obfuscation), excessive subdomain nesting, punycode/homograph labels, suspicious or lookalike TLDs, percent-encoded hostnames, plain http, URL-shortener hosts, credential/urgency keywords, hyphen-stacked hosts, non-standard ports, excessive length, and digit-heavy domains. Returns a MINIMAL/LOW/MEDIUM/HIGH/CRITICAL rating, a 0-100 composite score, and each finding with its severity. Deterministic: the same URL always yields the same rating. A clean rating means no structural red flags, not a guarantee the URL is safe.",
        parameters = schema_json()
    ),
)]
impl UrlSafetyInspect {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "url-safety-inspect", |a: Args| {
            run(&a.url).map_err(SkillError::InvalidArgs)
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
                    "url": { "type": "string", "description": "The URL to inspect (e.g. https://secure-login.example.tk/verify). A scheme is optional; whitespace is trimmed. Only the URL's structure is examined — no request is made to the site." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
