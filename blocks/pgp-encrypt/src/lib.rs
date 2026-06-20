//! gizza-ai/pgp-encrypt — encrypt a message to one or more OpenPGP public keys,
//! producing an ASCII-armored block. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    keys: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The plaintext message to encrypt."),
        )
        .param(Param::string("keys").required().describe(
            "One or more recipient OpenPGP public keys, ASCII-armored (each a full -----BEGIN PGP PUBLIC KEY BLOCK----- ... block). Paste several to encrypt to multiple recipients.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PgpEncrypt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pgp-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt a message to OpenPGP public keys",
    skill(
        description = "Encrypt a message to one or more OpenPGP (PGP/GPG) public keys, producing an ASCII-armored '-----BEGIN PGP MESSAGE-----' block that only the holders of the matching private keys can decrypt. Accepts one or more ASCII-armored public keys (paste several to encrypt to multiple recipients); each is encrypted to its encryption subkey (or encryption-capable primary). Uses AES-256. Runs locally — the message and keys never leave the device.",
        parameters = schema_json()
    ),
)]
impl PgpEncrypt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pgp-encrypt", |a: Args| {
            gizza_ai_pgp_encrypt_core::run(&a.message, &a.keys).map_err(SkillError::InvalidArgs)
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
                    "message": { "type": "string", "description": "The plaintext message to encrypt." },
                    "keys": { "type": "string", "description": "One or more recipient OpenPGP public keys, ASCII-armored (each a full -----BEGIN PGP PUBLIC KEY BLOCK----- ... block). Paste several to encrypt to multiple recipients." }
                },
                "required": ["message", "keys"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
