//! gizza-ai/pgp-sign — create a detached or clear-signed OpenPGP signature over a
//! message with a private key. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pgp_sign_core::Mode;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    private_key: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    passphrase: String,
}

fn default_mode() -> String {
    "detached".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The message to sign."),
        )
        .param(Param::string("private_key").required().describe(
            "Your OpenPGP private key, ASCII-armored (-----BEGIN PGP PRIVATE KEY BLOCK----- ... block).",
        ))
        .param(
            Param::enumv("mode", ["detached", "clearsign"]).default("detached").describe(
                "detached (default) -> a standalone PGP SIGNATURE block that verifies the original message; clearsign -> an inline PGP SIGNED MESSAGE keeping the readable text.",
            ),
        )
        .param(
            Param::string("passphrase")
                .describe("Passphrase to unlock the private key, if it is protected. Omit for an unprotected key."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PgpSign;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pgp-sign",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sign a message with an OpenPGP private key",
    skill(
        description = "Create an OpenPGP (PGP/GPG) signature over a message using your ASCII-armored private key. mode=detached (default) produces a standalone '-----BEGIN PGP SIGNATURE-----' block that verifies the original, unmodified message; mode=clearsign produces an inline '-----BEGIN PGP SIGNED MESSAGE-----' block that keeps the text readable with the signature appended. Supply a passphrase if the key is protected. Signs with the key's primary key; uses the key's preferred hash. Runs locally — the private key and message never leave the device.",
        parameters = schema_json()
    ),
)]
impl PgpSign {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pgp-sign", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            gizza_ai_pgp_sign_core::run(&a.message, &a.private_key, mode, &a.passphrase)
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "The message to sign." },
                    "private_key": { "type": "string", "description": "Your OpenPGP private key, ASCII-armored (-----BEGIN PGP PRIVATE KEY BLOCK----- ... block)." },
                    "mode": { "type": "string", "enum": ["detached", "clearsign"], "default": "detached", "description": "detached (default) -> a standalone PGP SIGNATURE block that verifies the original message; clearsign -> an inline PGP SIGNED MESSAGE keeping the readable text." },
                    "passphrase": { "type": "string", "description": "Passphrase to unlock the private key, if it is protected. Omit for an unprotected key." }
                },
                "required": ["message", "private_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
