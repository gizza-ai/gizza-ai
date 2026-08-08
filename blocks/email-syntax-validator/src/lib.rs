//! gizza-ai/email-syntax-validator — validate email syntax and risk signals.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
    #[serde(default = "default_true")]
    check_disposable: bool,
    #[serde(default = "default_true")]
    check_role: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("email").required().describe("Email address to validate. Surrounding whitespace, mailto: and Name <addr> wrappers are accepted and normalized before validation."))
        .param(Param::boolean("check_disposable").default(true).describe("Flag known disposable or temporary-inbox domains using the bundled offline domain list. Default true."))
        .param(Param::boolean("check_role").default(true).describe("Flag role-based local parts such as admin, info, sales, support, postmaster or abuse. Default true."))
        .param(Param::enumv("format", ["report", "summary", "json"]).default("report").describe("Output format: report for a multiline explanation, summary for one line, or json for machine-readable fields. Default report."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-syntax-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate email syntax, disposable domains and role addresses",
    skill(
        description = "Validate an email address with offline syntax checks, optional disposable-domain detection and optional role-based mailbox detection. Returns a human report, summary or JSON without DNS or SMTP network probes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-syntax-validator", |a: Args| {
            gizza_ai_email_syntax_validator_core::run(
                &a.email,
                a.check_disposable,
                a.check_role,
                &a.format,
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
                    "email": { "type": "string", "description": "Email address to validate. Surrounding whitespace, mailto: and Name <addr> wrappers are accepted and normalized before validation." },
                    "check_disposable": { "type": "boolean", "default": true, "description": "Flag known disposable or temporary-inbox domains using the bundled offline domain list. Default true." },
                    "check_role": { "type": "boolean", "default": true, "description": "Flag role-based local parts such as admin, info, sales, support, postmaster or abuse. Default true." },
                    "format": { "type": "string", "enum": ["report", "summary", "json"], "default": "report", "description": "Output format: report for a multiline explanation, summary for one line, or json for machine-readable fields. Default report." }
                },
                "required": ["email"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
