//! gizza-ai/pii-tokenize — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Replaces detected PII with
//! deterministic, format-preserving pseudonyms (same value → same token), so the
//! text stays linkable while de-identified. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, ToolDescriptor};
use gizza_ai_pii_tokenize_core::tokenize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    secret: String,
    #[serde(default = "default_preserve_email_domain")]
    preserve_email_domain: bool,
}

fn default_preserve_email_domain() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan and tokenize."),
        )
        .param(
            Param::string("secret")
                .default("")
                .describe("Optional secret key that scopes the deterministic mapping. The same secret always produces the same tokens (so tokenized data stays joinable across runs); a different secret produces a different, unlinkable set. Leave blank to use a built-in default key."),
        )
        .param(
            Param::boolean("preserve_email_domain")
                .default(true)
                .describe("When true (default), keep the part after @ in email addresses and only pseudonymize the local part (useful for segmenting by provider). When false, pseudonymize the whole address."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PiiTokenize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pii-tokenize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Replace PII with deterministic, format-preserving tokens",
    skill(
        description = "Replace personally-identifiable information (PII) in text with deterministic, format-preserving pseudonyms: email addresses, phone numbers, IPv4/IPv6 addresses, credit-card numbers (Luhn-validated) and US SSN-like numbers. Each value is substituted character-by-character (digit→digit, letter→same-case letter, punctuation kept), so tokens keep the original shape and length. The mapping is deterministic — the same input value always maps to the same token, so tokenized data stays linkable/joinable — but not reversible: it is keyed HMAC-SHA256, not a lookup vault. Card tokens stay Luhn-valid; IPv4 octets stay 0-255; IPv6 groups stay valid hex. secret scopes the mapping (same secret → same tokens; blank uses a built-in key). preserve_email_domain (default true) keeps the email domain and only pseudonymizes the local part. Returns the tokenized text plus per-category counts. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl PiiTokenize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pii-tokenize", |a: Args| {
            Ok(tokenize(&a.text, &a.secret, a.preserve_email_domain))
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
                    "text": { "type": "string", "description": "The text to scan and tokenize." },
                    "secret": { "type": "string", "default": "", "description": "Optional secret key that scopes the deterministic mapping. The same secret always produces the same tokens (so tokenized data stays joinable across runs); a different secret produces a different, unlinkable set. Leave blank to use a built-in default key." },
                    "preserve_email_domain": { "type": "boolean", "default": true, "description": "When true (default), keep the part after @ in email addresses and only pseudonymize the local part (useful for segmenting by provider). When false, pseudonymize the whole address." }
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
