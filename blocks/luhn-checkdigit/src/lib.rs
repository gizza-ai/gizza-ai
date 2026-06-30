//! gizza-ai/luhn-checkdigit — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_luhn_checkdigit_core::check_digit;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    number: String,
}

#[derive(Serialize)]
struct Resp {
    check_digit: u8,
    payload: String,
    full_number: String,
    length: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("number")
            .required()
            .describe("The partial number WITHOUT its check digit (the payload). Spaces and dashes are ignored."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/luhn-checkdigit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the Luhn check digit for a partial number",
    skill(
        description = "Compute the Luhn (mod-10) check digit for a partial number — the payload WITHOUT its check digit — and return the completed, valid number. Every input digit is treated as payload (unlike validation, which treats the last digit as the check digit). Used to generate valid credit/debit card, IMEI, and ID numbers. Spaces and dashes are ignored. Returns the single check digit (0-9), the cleaned payload, and the full number.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "luhn-checkdigit", |a: Args| {
            let r = check_digit(&a.number).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                check_digit: r.check_digit,
                payload: r.payload,
                full_number: r.full_number,
                length: r.length,
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
                    "number": { "type": "string", "description": "The partial number WITHOUT its check digit (the payload). Spaces and dashes are ignored." }
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
