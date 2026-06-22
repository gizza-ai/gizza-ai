//! gizza-ai/iban-validator — validate an IBAN (ISO 13616) with the mod-97
//! checksum and parse out country, bank and account components. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_iban_validator_core::validate;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    iban: String,
}

#[derive(Serialize)]
struct Resp {
    valid: bool,
    normalized: String,
    formatted: String,
    country_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    check_digits: String,
    bban: String,
    length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    bank_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_number: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("iban")
            .required()
            .describe("The IBAN to validate, e.g. \"GB82 WEST 1234 5698 7654 32\". Spaces are ignored and letters are upper-cased."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IbanValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/iban-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate an IBAN with the mod-97 checksum",
    skill(
        description = "Validate an IBAN (International Bank Account Number, ISO 13616): checks the two-letter country code, the country-specific length, and the ISO 7064 mod-97 checksum. Spaces are ignored. Returns whether it's valid, the normalized and 4-block-formatted IBAN, the country and code, the check digits, the BBAN, the total length, and — for common countries — the bank code and account number parsed out of the BBAN.",
        parameters = schema_json()
    ),
)]
impl IbanValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "iban-validator", |a: Args| {
            let r = validate(&a.iban).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                valid: r.valid,
                normalized: r.normalized,
                formatted: r.formatted,
                country_code: r.country_code,
                country: r.country,
                check_digits: r.check_digits,
                bban: r.bban,
                length: r.length,
                bank_code: r.bank_code,
                branch_code: r.branch_code,
                account_number: r.account_number,
            })
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
                    "iban": { "type": "string", "description": "The IBAN to validate, e.g. \"GB82 WEST 1234 5698 7654 32\". Spaces are ignored and letters are upper-cased." }
                },
                "required": ["iban"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
