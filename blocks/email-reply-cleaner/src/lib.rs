//! gizza-ai/email-reply-cleaner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    remove_quotes: bool,
    #[serde(default = "default_true")]
    remove_reply_chain: bool,
    #[serde(default = "default_true")]
    remove_signature: bool,
    #[serde(default = "default_true")]
    collapse_blank_lines: bool,
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The replied or forwarded plain-text email to clean. Paste the whole body; the tool returns just the fresh message."),
        )
        .param(
            Param::boolean("remove_quotes")
                .default(true)
                .describe("Drop lines that begin with the `>` quote marker (including nested `>>`). Default true."),
        )
        .param(
            Param::boolean("remove_reply_chain")
                .default(true)
                .describe("Cut the quoted reply chain: the attribution ('On … wrote:'), Outlook '-----Original Message-----' / 'From:' header block, or a forwarded-message rule, and everything below it. Default true."),
        )
        .param(
            Param::boolean("remove_signature")
                .default(true)
                .describe("Cut the signature block: the RFC 3676 '-- ' delimiter or a mobile/app footer ('Sent from my iPhone', 'Get Outlook for …'), and everything below it. Default true."),
        )
        .param(
            Param::boolean("collapse_blank_lines")
                .default(true)
                .describe("Collapse runs of blank lines to a single blank line and trim leading/trailing blank lines. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-reply-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip quotes, reply chains, and signatures from a replied or forwarded email.",
    skill(
        description = "Extract just the fresh message from a replied or forwarded plain-text email. Four deterministic passes, each toggleable: `remove_quotes` drops `>`-prefixed (and nested `>>`) quote lines; `remove_reply_chain` cuts the attribution ('On … wrote:'), Outlook '-----Original Message-----' / 'From:' header block, or a forwarded-message rule and everything below; `remove_signature` cuts the RFC 3676 '-- ' delimiter or a mobile/app footer ('Sent from my iPhone', 'Get Outlook for …') and everything below; `collapse_blank_lines` folds blank-line runs and trims the edges. All default on. English-focused attribution detection; runs locally, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-reply-cleaner", |a: Args| {
            gizza_ai_email_reply_cleaner_core::run(
                &a.text,
                a.remove_quotes,
                a.remove_reply_chain,
                a.remove_signature,
                a.collapse_blank_lines,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The replied or forwarded plain-text email to clean. Paste the whole body; the tool returns just the fresh message." },
                    "remove_quotes": { "type": "boolean", "default": true, "description": "Drop lines that begin with the `>` quote marker (including nested `>>`). Default true." },
                    "remove_reply_chain": { "type": "boolean", "default": true, "description": "Cut the quoted reply chain: the attribution ('On … wrote:'), Outlook '-----Original Message-----' / 'From:' header block, or a forwarded-message rule, and everything below it. Default true." },
                    "remove_signature": { "type": "boolean", "default": true, "description": "Cut the signature block: the RFC 3676 '-- ' delimiter or a mobile/app footer ('Sent from my iPhone', 'Get Outlook for …'), and everything below it. Default true." },
                    "collapse_blank_lines": { "type": "boolean", "default": true, "description": "Collapse runs of blank lines to a single blank line and trim leading/trailing blank lines. Default true." }
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
