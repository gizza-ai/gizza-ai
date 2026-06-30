//! gizza-ai/pgp-key-info — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    key: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(Param::string("key").required().describe(
        "ASCII-armored OpenPGP public or private key block to inspect.",
    ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pgp-key-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect ASCII-armored OpenPGP key metadata.",
    skill(
        description = "Inspect an ASCII-armored OpenPGP public or private key and return fingerprint, key ID, algorithm, user IDs, dates, and subkeys as JSON.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pgp-key-info", |a: Args| {
            let info = gizza_ai_pgp_key_info_core::inspect(&a.key)
                .map_err(SkillError::InvalidArgs)?;
            serde_json::to_value(info).map_err(|e| SkillError::Serialize(e.to_string()))
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored = serde_json::json!({
            "type": "object",
            "required": ["key"],
            "properties": {
                "key": {
                    "type": "string",
                    "description": "ASCII-armored OpenPGP public or private key block to inspect."
                }
            },
            "additionalProperties": false
        });
        assert_eq!(derived, authored);
    }
}
