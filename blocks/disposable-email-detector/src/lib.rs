//! gizza-ai/disposable-email-detector — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. No host calls
//! — runs entirely inside the WASM sandbox against a built-in domain list (no
//! DNS/network lookup).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("email")
            .required()
            .describe("The email address to check — or a bare domain (domain.tld). A 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped before checking."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/disposable-email-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag whether an email uses a known disposable / throwaway-inbox domain.",
    skill(
        description = "Flag whether an email address (or a bare domain) uses a known disposable, temporary, or throwaway-inbox provider such as Mailinator, Guerrilla Mail, 10 Minute Mail, YOPmail, or Temp-Mail. Runs entirely offline against a built-in list of well-known disposable domains (and their alias domains and subdomains) plus a conservative throwaway-keyword heuristic — it never performs a DNS, MX, or SMTP lookup. Accepts a full address (name@domain.tld) or just a domain; unwraps a 'Name <addr>' / 'mailto:addr' wrapper first. Reports the parsed domain, whether it is disposable, the specific matched domain when applicable, and a short reason. A 'disposable: no' result means the domain is not on the known list, which is a signal rather than a guarantee that the mailbox is real.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "disposable-email-detector", |a: Args| {
            Ok::<String, SkillError>(gizza_ai_disposable_email_detector_core::report(&a.email))
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
                    "email": { "type": "string", "description": "The email address to check — or a bare domain (domain.tld). A 'Name <addr>', 'mailto:addr', or angle-bracketed form is accepted and unwrapped before checking." }
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
