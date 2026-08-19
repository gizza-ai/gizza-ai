//! gizza-ai/pgp-decrypt — chat skill block on the shared tool abstraction.
//! Decrypts an ASCII-armored OpenPGP message with either a private key or the
//! password it was symmetrically encrypted with, and reports (and optionally
//! verifies) an embedded signature. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use serde_json::Value;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    /// The recipient's armored private key (public-key-encrypted messages).
    #[serde(default)]
    private_key: String,
    /// Unlocks a protected private key, or is the password of a symmetrically
    /// encrypted message.
    #[serde(default)]
    passphrase: String,
    /// The signer's armored public key, to verify an embedded signature.
    #[serde(default)]
    public_key: String,
    #[serde(default)]
    output_format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("message").required().describe(
            "The encrypted OpenPGP message to decrypt, ASCII-armored: the whole '-----BEGIN PGP MESSAGE-----' ... '-----END PGP MESSAGE-----' block.",
        ))
        .param(Param::string("private_key").describe(
            "The recipient's OpenPGP private key, ASCII-armored ('-----BEGIN PGP PRIVATE KEY BLOCK-----' block). Required for a message encrypted to a public key; leave blank for a password-encrypted ('gpg --symmetric') message.",
        ))
        .param(Param::string("passphrase").describe(
            "The passphrase that unlocks the private key, or — for a password-encrypted message — the password the message itself was encrypted with. Leave blank for an unprotected key.",
        ))
        .param(Param::string("public_key").describe(
            "Optional: the signer's OpenPGP public key, ASCII-armored. Supply it to verify the signature of a message that was signed as well as encrypted; without it a signature is reported but not checked.",
        ))
        .param(
            Param::enumv("output_format", ["auto", "text", "base64", "hex"])
                .default("auto")
                .describe(
                    "How to render the decrypted bytes: 'auto' shows UTF-8 text and falls back to base64 for binary payloads, 'text' always shows text (and errors on non-UTF-8 data), 'base64' and 'hex' always encode.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PgpDecrypt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pgp-decrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decrypt an OpenPGP (PGP/GPG) message",
    skill(
        description = "Decrypt an ASCII-armored OpenPGP (PGP/GPG) '-----BEGIN PGP MESSAGE-----' block. Handles both encryption shapes and picks between them from the message's own session-key packets: public-key encryption (supply the recipient's armored 'private_key', plus its 'passphrase' if it is protected) and password/symmetric encryption made by 'gpg --symmetric' (supply only the 'passphrase'). Compressed payloads are decompressed automatically; an embedded signature is reported and, when the signer's 'public_key' is supplied, verified. Returns {plaintext, encryption, output_format, bytes, binary, compressed, decrypted_with_key_id, recipient_key_ids, signature}: encryption is 'public-key' or 'password', output_format is the rendering actually used ('text', 'base64' or 'hex' — 'auto' shows text and falls back to base64 for binary data), and signature carries {valid, signer_key_id, signer_fingerprint, signer_user_id, signed_at, hash_algorithm, note} with valid null when no public key was given. Armored input is capped at 4 MiB. Runs locally — the message, keys and passphrase never leave the device.",
        parameters = schema_json()
    ),
)]
impl PgpDecrypt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pgp-decrypt", |a: Args| {
            let fmt = gizza_ai_pgp_decrypt_core::OutputFormat::parse(
                a.output_format.as_deref().unwrap_or("auto"),
            )
            .map_err(SkillError::InvalidArgs)?;
            let res = gizza_ai_pgp_decrypt_core::run(
                &a.message,
                &a.private_key,
                &a.passphrase,
                &a.public_key,
                fmt,
            )
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
                    "message": { "type": "string", "description": "The encrypted OpenPGP message to decrypt, ASCII-armored: the whole '-----BEGIN PGP MESSAGE-----' ... '-----END PGP MESSAGE-----' block." },
                    "private_key": { "type": "string", "description": "The recipient's OpenPGP private key, ASCII-armored ('-----BEGIN PGP PRIVATE KEY BLOCK-----' block). Required for a message encrypted to a public key; leave blank for a password-encrypted ('gpg --symmetric') message." },
                    "passphrase": { "type": "string", "description": "The passphrase that unlocks the private key, or — for a password-encrypted message — the password the message itself was encrypted with. Leave blank for an unprotected key." },
                    "public_key": { "type": "string", "description": "Optional: the signer's OpenPGP public key, ASCII-armored. Supply it to verify the signature of a message that was signed as well as encrypted; without it a signature is reported but not checked." },
                    "output_format": {
                        "type": "string",
                        "enum": ["auto", "text", "base64", "hex"],
                        "default": "auto",
                        "description": "How to render the decrypted bytes: 'auto' shows UTF-8 text and falls back to base64 for binary payloads, 'text' always shows text (and errors on non-UTF-8 data), 'base64' and 'hex' always encode."
                    }
                },
                "required": ["message"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page form reads manifest.json's `tool.parameters`, so the enum there
    /// must stay in sync with the descriptor or the field renders as a text box.
    #[test]
    fn manifest_parameters_match_the_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            manifest["tool"]["parameters"]["properties"]["output_format"]["enum"],
            derived["properties"]["output_format"]["enum"],
            "manifest enum drifted from the descriptor — rerun scripts/sync-tool-manifest.py"
        );
        assert_eq!(
            manifest["tool"]["parameters"]["required"],
            derived["required"],
            "manifest required-list drifted from the descriptor"
        );
    }
}
