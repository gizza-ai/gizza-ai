//! gizza-ai/ioc-extract — extract indicators of compromise (IOCs) from text.
//!
//! Thin chat-skill wrapper around `gizza-ai-ioc-extract-core`. The chat schema is
//! single-sourced from `descriptor()` (shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    types: String,
    #[serde(default)]
    defang: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan for indicators of compromise — a log line, email, alert or threat report. Defanged input (hxxp[://]…, 1[.]2[.]3[.]4, bad[at]evil[dot]com) is refanged before matching."),
        )
        .param(
            Param::string("types")
                .default("all")
                .describe("Comma-separated list of indicator types to extract: any of 'ipv4', 'ipv6', 'url', 'domain', 'email', 'md5', 'sha1', 'sha256', 'sha512' (also 'hash' for all four hash types). 'all' (default) extracts every type."),
        )
        .param(
            Param::boolean("defang")
                .default(false)
                .describe("When true, re-defang the extracted indicators in the output (evil[.]com, hxxp[://]…, bad[at]evil[.]com) so the result is safe to paste into a report or ticket. Default false emits the real, clickable indicators."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ioc-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract IOCs (IPs, URLs, domains, emails, hashes) from text.",
    skill(
        description = "Extract indicators of compromise (IOCs) from an arbitrary block of text — a log line, email, alert, or threat-intel report. It pulls out IPv4 and IPv6 addresses, URLs (http/https/ftp), domains/hostnames, email addresses, and file hashes (MD5, SHA-1, SHA-256, SHA-512), then de-duplicates, sorts, and groups them by type. Defanged input is recognized and refanged before matching, so you can paste indicators straight out of a CTI report (it understands hxxp[://]…, 1[.]2[.]3[.]4, bad[at]evil[dot]com, plus the round () and curly {} bracket variants). Use the 'types' parameter to restrict to specific categories (e.g. 'ipv4,url,sha256' or 'hash' for all hash types; 'all' is the default). Set 'defang' to true to re-defang the extracted indicators in the output so the result itself is safe to paste back into a ticket or email. URLs and email addresses are excluded from the 'domain' group so categories don't overlap.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ioc-extract", |a: Args| {
            gizza_ai_ioc_extract_core::extract(&a.text, &a.types, a.defang)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to scan for indicators of compromise — a log line, email, alert or threat report. Defanged input (hxxp[://]…, 1[.]2[.]3[.]4, bad[at]evil[dot]com) is refanged before matching." },
                    "types": {
                        "type": "string",
                        "default": "all",
                        "description": "Comma-separated list of indicator types to extract: any of 'ipv4', 'ipv6', 'url', 'domain', 'email', 'md5', 'sha1', 'sha256', 'sha512' (also 'hash' for all four hash types). 'all' (default) extracts every type."
                    },
                    "defang": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, re-defang the extracted indicators in the output (evil[.]com, hxxp[://]…, bad[at]evil[.]com) so the result is safe to paste into a report or ticket. Default false emits the real, clickable indicators."
                    }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
