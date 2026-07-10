//! gizza-ai/ed25519-sign-verify — sign a message with an Ed25519 private key or
//! verify a signature with the matching public key. Chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure-Rust (deterministic,
//! no RNG) → runs on all backends. Surfaces: chat + CLI + page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ed25519_sign_verify_core::{process, MsgEncoding, Operation};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_operation")]
    operation: String,
    message: String,
    #[serde(default = "default_encoding")]
    message_encoding: String,
    key: String,
    #[serde(default)]
    signature: String,
}

fn default_operation() -> String {
    "sign".to_string()
}
fn default_encoding() -> String {
    "utf8".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("operation", ["sign", "verify"]).default("sign").describe(
                "sign: create an Ed25519 signature for the message with a private key. verify: check a signature against the message with a public key.",
            ),
        )
        .param(
            Param::string("message")
                .required()
                .describe("The message to sign or verify. Interpreted using message_encoding."),
        )
        .param(
            Param::enumv("message_encoding", ["utf8", "hex", "base64"]).default("utf8").describe(
                "How to read the message into bytes: utf8 (plain text), hex, or base64 (for binary payloads).",
            ),
        )
        .param(Param::string("key").required().describe(
            "For sign: your Ed25519 private key. For verify: the signer's public key. Accepts PEM (PKCS#8 private / SPKI public), or a raw 32-byte key as hex or base64 (a private key may also be a 64-byte seed||public keypair).",
        ))
        .param(Param::string("signature").describe(
            "Required for verify: the signature to check, as hex or base64 (the raw 64 bytes). Ignored for sign.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Ed25519SignVerify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ed25519-sign-verify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sign and verify messages with Ed25519 keys",
    skill(
        description = "Sign a message with an Ed25519 private key, or verify an Ed25519 signature with the matching public key. Ed25519 (EdDSA over Curve25519, RFC 8032) signatures are deterministic — the same key + message always yields the same 64-byte signature, with no repeated-nonce risk. Keys are accepted as PEM (PKCS#8 private / SPKI public) or as a raw 32-byte key in hex or base64; the message can be plain text (utf8), hex, or base64. Signing returns the signature in hex and base64 plus the derived public key; verifying returns whether the signature is valid. Runs locally — keys and messages never leave the device.",
        parameters = schema_json()
    ),
)]
impl Ed25519SignVerify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ed25519-sign-verify", |a: Args| {
            let op = Operation::parse(&a.operation).map_err(SkillError::InvalidArgs)?;
            let enc = MsgEncoding::parse(&a.message_encoding).map_err(SkillError::InvalidArgs)?;
            process(op, &a.message, enc, &a.key, &a.signature).map_err(SkillError::InvalidArgs)
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
                    "operation": { "type": "string", "enum": ["sign", "verify"], "default": "sign", "description": "sign: create an Ed25519 signature for the message with a private key. verify: check a signature against the message with a public key." },
                    "message": { "type": "string", "description": "The message to sign or verify. Interpreted using message_encoding." },
                    "message_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to read the message into bytes: utf8 (plain text), hex, or base64 (for binary payloads)." },
                    "key": { "type": "string", "description": "For sign: your Ed25519 private key. For verify: the signer's public key. Accepts PEM (PKCS#8 private / SPKI public), or a raw 32-byte key as hex or base64 (a private key may also be a 64-byte seed||public keypair)." },
                    "signature": { "type": "string", "description": "Required for verify: the signature to check, as hex or base64 (the raw 64 bytes). Ignored for sign." }
                },
                "required": ["message", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
