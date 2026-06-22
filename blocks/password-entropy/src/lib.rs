//! gizza-ai/password-entropy — estimate password strength in bits + flag
//! weaknesses. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends (runs locally; the
//! password never leaves the device).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_password_entropy_core::analyze;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    password: String,
}

#[derive(Serialize)]
struct Resp {
    bits: f64,
    charset_size: u32,
    length: usize,
    rating: String,
    crack_time: String,
    warnings: Vec<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("password")
            .required()
            .describe("The password to analyze. It is processed locally and never stored or sent anywhere."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PasswordEntropy;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/password-entropy",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate password strength (entropy bits)",
    skill(
        description = "Estimate a password's strength in bits of entropy from its character set (lower/upper/digits/symbols) and length, give a rating (Very weak..Very strong) and a rough offline crack-time estimate, and flag weaknesses (too short, single character type, common password, repeated/sequential patterns). The password is analyzed locally and never stored or transmitted.",
        parameters = schema_json()
    )
)]
impl PasswordEntropy {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "password-entropy", |a: Args| {
            let s = analyze(&a.password).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                bits: s.bits,
                charset_size: s.charset_size,
                length: s.length,
                rating: s.rating.to_string(),
                crack_time: s.crack_time,
                warnings: s.warnings,
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
                    "password": { "type": "string", "description": "The password to analyze. It is processed locally and never stored or sent anywhere." }
                },
                "required": ["password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
