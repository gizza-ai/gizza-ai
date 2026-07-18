//! gizza-ai/iban-extractor-validator — scan free-form text for IBANs and
//! validate each with the ISO 13616 mod-97 checksum. Chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_iban_extractor_validator_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("The text to scan for IBANs — an invoice, email, bank statement, or any pasted block. IBANs may be grouped in 4-character blocks or written contiguously; spaces and surrounding punctuation are handled."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IbanExtractorValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/iban-extractor-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find IBANs in text and validate each with mod-97",
    skill(
        description = "Scan free-form text (invoices, emails, statements, logs) for IBANs and validate each with the ISO 13616 mod-97 checksum plus the country-specific length. Each candidate is anchored at a two-letter country code + two check digits, sized to that country's registered IBAN length, and normalized (spaces removed, upper-cased) before checking. Returns the valid IBANs (with country, 4-block formatting, and — for common countries — the bank code and account number parsed from the BBAN) and, separately, structurally IBAN-shaped candidates that failed the checksum, both deduplicated in first-seen order, plus counts. Does not look up bank names/BIC or verify that an account is open. Runs locally.",
        parameters = schema_json()
    ),
)]
impl IbanExtractorValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "iban-extractor-validator", |a: Args| {
            Ok::<_, SkillError>(extract(&a.text))
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
                    "text": { "type": "string", "description": "The text to scan for IBANs — an invoice, email, bank statement, or any pasted block. IBANs may be grouped in 4-character blocks or written contiguously; spaces and surrounding punctuation are handled." }
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
