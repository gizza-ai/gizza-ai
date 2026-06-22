//! gizza-ai/email-validator — validate an email address (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-email-validator-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox (syntax-only; never touches DNS/SMTP).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("email")
            .required()
            .describe("The email address to validate. A 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped before validation."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmailValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate an email address.",
    skill(
        description = "Validate an email address against RFC 5321/5322 syntax rules and flag common typos and formatting issues. This is syntax-only and never touches the network (no DNS/MX or SMTP checks). It parses local@domain; reports whether the address is syntactically valid; lists hard errors (missing or duplicated '@', empty/over-long local part or domain, leading/trailing or consecutive dots, illegal characters, a domain with no dot or bad labels); and lists soft warnings for likely mistakes (a misspelled popular domain such as gmial.com, a misspelled TLD such as '.con', an all-numeric TLD, surrounding whitespace, a quoted local part, an IP-address literal). When a typo is detected it returns a best-guess corrected address. Unwraps a 'Name <addr>' / 'mailto:addr' wrapper before validating.",
        parameters = schema_json()
    ),
)]
impl EmailValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "email-validator", |a: Args| {
            Ok::<String, SkillError>(gizza_ai_email_validator_core::report(&a.email))
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
                    "email": { "type": "string", "description": "The email address to validate. A 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped before validation." }
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
