//! gizza-ai/email-normalizer — canonicalizes an email address.
//!
//! Thin chat-skill wrapper around `gizza-ai-email-normalizer-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
    /// Lowercase the local part (before `@`). Default true. Set false to keep
    /// the local-part case for a case-sensitive server.
    #[serde(default = "default_true")]
    lowercase_local: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("email")
                .required()
                .describe("The email address to canonicalize (a 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped)."),
        )
        .param(
            Param::boolean("lowercase_local")
                .default(true)
                .describe("Lowercase the local part (before '@'). Default true — virtually every provider treats the local part case-insensitively. Set false to preserve the original case for a rare case-sensitive server."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmailNormalizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Canonicalize an email address to its deliverable form.",
    skill(
        description = "Canonicalize an email address to its deliverable form. Lowercases the domain (and, by default, the local part); for Gmail/Googlemail it strips all dots from the local part and folds googlemail.com to gmail.com; for Gmail, Outlook/Hotmail/Live, Yahoo, iCloud, Fastmail, Proton and other plus-tag providers it drops the '+tag' sub-address. Unwraps a 'Name <addr>' / 'mailto:addr' wrapper. Reports the canonical address, local part, domain, recognised provider, the stripped sub-address tag (if any), and whether dots were removed. Returns an error for a syntactically invalid address.",
        parameters = schema_json()
    ),
)]
impl EmailNormalizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "email-normalizer", |a: Args| {
            gizza_ai_email_normalizer_core::report(&a.email, a.lowercase_local)
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
                    "email": { "type": "string", "description": "The email address to canonicalize (a 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped)." },
                    "lowercase_local": { "type": "boolean", "default": true, "description": "Lowercase the local part (before '@'). Default true — virtually every provider treats the local part case-insensitively. Set false to preserve the original case for a rare case-sensitive server." }
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
