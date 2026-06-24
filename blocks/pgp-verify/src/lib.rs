//! gizza-ai/pgp-verify — chat skill block on the shared tool abstraction.
//! Verifies an OpenPGP (PGP/GPG) signature — detached or clearsigned — against a
//! message and the signer's public key, reporting validity + the signer key ID.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use serde_json::Value;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    signature: String,
    public_key: String,
    /// The original signed message (required for a detached signature; ignored
    /// for a clearsigned block, which embeds its own text).
    #[serde(default)]
    message: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("signature").required().describe(
            "The OpenPGP signature to verify, ASCII-armored: either a detached '-----BEGIN PGP SIGNATURE-----' block, or a clearsigned '-----BEGIN PGP SIGNED MESSAGE-----' block (which embeds the signed text).",
        ))
        .param(Param::string("public_key").required().describe(
            "The signer's OpenPGP public key, ASCII-armored ('-----BEGIN PGP PUBLIC KEY BLOCK-----' block).",
        ))
        .param(Param::string("message").describe(
            "The original signed message. Required for a detached signature (must be the exact, unmodified bytes that were signed); ignored for a clearsigned block, which carries its own text.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PgpVerify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pgp-verify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify an OpenPGP (PGP/GPG) signature",
    skill(
        description = "Verify an OpenPGP (PGP/GPG) signature against a message and the signer's public key. Accepts a detached signature ('-----BEGIN PGP SIGNATURE-----', paired with the original 'message') or a clearsigned block ('-----BEGIN PGP SIGNED MESSAGE-----', which embeds its own text so 'message' is ignored) — the shape is auto-detected. Tries the public key's primary key and every subkey, so subkey-signed messages still verify. Returns {valid, mode, signer_key_id, signer_fingerprint, signer_user_id, signed_at, hash_algorithm, signed_text, error}: valid is true only when the signature checks out; signer_user_id is the key's primary UID (name/email), signed_at is the signature's creation time, hash_algorithm is e.g. SHA256; on failure error explains why (tampered message or wrong key). Runs locally — the message and key never leave the device.",
        parameters = schema_json()
    ),
)]
impl PgpVerify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pgp-verify", |a: Args| {
            let res = gizza_ai_pgp_verify_core::run(&a.message, &a.signature, &a.public_key)
                .map_err(SkillError::InvalidArgs)?;
            Ok::<Value, SkillError>(res.to_json())
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "signature": { "type": "string", "description": "The OpenPGP signature to verify, ASCII-armored: either a detached '-----BEGIN PGP SIGNATURE-----' block, or a clearsigned '-----BEGIN PGP SIGNED MESSAGE-----' block (which embeds the signed text)." },
                    "public_key": { "type": "string", "description": "The signer's OpenPGP public key, ASCII-armored ('-----BEGIN PGP PUBLIC KEY BLOCK-----' block)." },
                    "message": { "type": "string", "description": "The original signed message. Required for a detached signature (must be the exact, unmodified bytes that were signed); ignored for a clearsigned block, which carries its own text." }
                },
                "required": ["signature", "public_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
