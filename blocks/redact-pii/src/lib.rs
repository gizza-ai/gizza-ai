//! gizza-ai/redact-pii — detect and mask PII (emails, phones, IPs, credit cards,
//! SSNs) in text. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_redact_pii_core::{redact, Style};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_style")]
    style: String,
}

fn default_style() -> String {
    "label".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan and redact."),
        )
        .param(
            Param::enumv("style", ["label", "mask"]).default("label").describe(
                "How to replace detected values: label (default) inserts a typed token like [EMAIL]; mask replaces each character with '*'.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RedactPii;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/redact-pii",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect and mask PII in text",
    skill(
        description = "Detect and redact personally-identifiable information (PII) in a block of text: email addresses, phone numbers, IPv4/IPv6 addresses, credit-card numbers (Luhn-validated to avoid false positives) and US SSN-like numbers. style=label (default) replaces each value with a typed token like [EMAIL]; style=mask replaces every character with '*'. Returns the redacted text plus per-category counts. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl RedactPii {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "redact-pii", |a: Args| {
            let style = Style::parse(&a.style).map_err(SkillError::InvalidArgs)?;
            Ok(redact(&a.text, style))
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
                    "text": { "type": "string", "description": "The text to scan and redact." },
                    "style": { "type": "string", "enum": ["label", "mask"], "default": "label", "description": "How to replace detected values: label (default) inserts a typed token like [EMAIL]; mask replaces each character with '*'." }
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
