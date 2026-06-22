//! gizza-ai/luhn-validate — validate a number with the Luhn algorithm. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_luhn_validate_core::check;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    number: String,
}

#[derive(Serialize)]
struct Resp {
    valid: bool,
    digits: String,
    length: usize,
    expected_check_digit: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("number")
            .required()
            .describe("The number to validate (credit card, IMEI, etc.). Spaces and dashes are ignored."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LuhnValidate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/luhn-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a number with the Luhn algorithm",
    skill(
        description = "Validate a number against the Luhn (mod-10) check-digit algorithm — used by credit/debit cards, IMEI numbers, and many ID schemes. Spaces and dashes are ignored. Returns whether it's valid, the cleaned digits and length, the check digit that would make it valid (to spot a typo or generate one), and a best-effort payment-card brand.",
        parameters = schema_json()
    )
)]
impl LuhnValidate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "luhn-validate", |a: Args| {
            let r = check(&a.number).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                valid: r.valid,
                digits: r.digits,
                length: r.length,
                expected_check_digit: r.expected_check_digit,
                brand: r.brand,
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
                    "number": { "type": "string", "description": "The number to validate (credit card, IMEI, etc.). Spaces and dashes are ignored." }
                },
                "required": ["number"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
