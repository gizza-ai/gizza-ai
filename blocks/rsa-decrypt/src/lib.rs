//! gizza-ai/rsa-decrypt — decrypt base64/hex RSA ciphertext with a private key
//! (OAEP or PKCS#1 v1.5). Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rsa_decrypt_core::{decrypt, CipherEncoding, Hash, OutputEncoding, Padding};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ciphertext: String,
    private_key: String,
    #[serde(default)]
    passphrase: String,
    #[serde(default = "default_padding")]
    padding: String,
    #[serde(default = "default_hash")]
    hash: String,
    #[serde(default = "default_ciphertext_encoding")]
    ciphertext_encoding: String,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
}

fn default_padding() -> String {
    "oaep".to_string()
}
fn default_hash() -> String {
    "sha256".to_string()
}
fn default_ciphertext_encoding() -> String {
    "auto".to_string()
}
fn default_output_encoding() -> String {
    "utf8".to_string()
}

#[derive(Serialize)]
struct Resp {
    plaintext: String,
    plaintext_bytes: usize,
    padding: String,
    hash: String,
    output_encoding: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ciphertext")
                .required()
                .describe("The RSA ciphertext to decrypt, base64 (e.g. 'Q1rN…AA==', standard or URL-safe) or hex. Exactly one RSA block — 256 bytes for a 2048-bit key, 512 for 4096-bit."),
        )
        .param(Param::string("private_key").required().describe(
            "Your RSA private key, PEM-encoded: PKCS#8 '-----BEGIN PRIVATE KEY-----', PKCS#1 '-----BEGIN RSA PRIVATE KEY-----', or passphrase-protected '-----BEGIN ENCRYPTED PRIVATE KEY-----'.",
        ))
        .param(Param::string("passphrase").default("").describe(
            "Passphrase that unlocks an encrypted PKCS#8 ('BEGIN ENCRYPTED PRIVATE KEY') key, e.g. 'hunter2'. Leave empty for an unencrypted key.",
        ))
        .param(
            Param::enumv("padding", ["oaep", "pkcs1v15"]).default("oaep").describe(
                "Padding the ciphertext was encrypted with: oaep (default, RSAES-OAEP) or pkcs1v15 (legacy RSAES-PKCS1-v1_5). Must match the sender exactly.",
            ),
        )
        .param(
            Param::enumv("hash", ["sha256", "sha384", "sha512"]).default("sha256").describe(
                "OAEP digest (MGF1 + label hash) the sender used; sha256 (default), sha384, or sha512. Must match exactly. Ignored for pkcs1v15.",
            ),
        )
        .param(
            Param::enumv("ciphertext_encoding", ["auto", "base64", "hex"]).default("auto").describe(
                "How the ciphertext is encoded: auto (default — hex if it is all hex digits, else base64), base64, or hex.",
            ),
        )
        .param(
            Param::enumv("output_encoding", ["utf8", "hex", "base64"]).default("utf8").describe(
                "How to render the recovered plaintext: utf8 (default, readable text), hex, or base64 (use these for binary payloads such as a wrapped AES key).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RsaDecrypt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rsa-decrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decrypt RSA ciphertext with a private key",
    skill(
        description = "Decrypt RSA ciphertext with an RSA private key and return the plaintext. padding=oaep (default, RSAES-OAEP) or pkcs1v15 (legacy RSAES-PKCS1-v1_5), and hash=sha256 (default), sha384 or sha512 selects the OAEP digest — both must match what the sender used. The ciphertext is base64 or hex (ciphertext_encoding=auto detects it); the private key is PEM (PKCS#8, PKCS#1, or passphrase-protected PKCS#8 via passphrase). output_encoding=utf8 (default), hex, or base64 renders the plaintext — use hex/base64 for binary payloads such as a wrapped AES key. Inverse of rsa-encrypt. Runs locally — the private key never leaves the device.",
        parameters = schema_json()
    ),
)]
impl RsaDecrypt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rsa-decrypt", |a: Args| {
            let padding = Padding::parse(&a.padding).map_err(SkillError::InvalidArgs)?;
            let hash = Hash::parse(&a.hash).map_err(SkillError::InvalidArgs)?;
            let cipher_encoding =
                CipherEncoding::parse(&a.ciphertext_encoding).map_err(SkillError::InvalidArgs)?;
            let output_encoding =
                OutputEncoding::parse(&a.output_encoding).map_err(SkillError::InvalidArgs)?;
            let out = decrypt(
                &a.ciphertext,
                &a.private_key,
                &a.passphrase,
                padding,
                hash,
                cipher_encoding,
                output_encoding,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                plaintext: out.plaintext,
                plaintext_bytes: out.plaintext_bytes,
                padding: a.padding,
                hash: a.hash,
                output_encoding: a.output_encoding,
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
                    "ciphertext": { "type": "string", "description": "The RSA ciphertext to decrypt, base64 (e.g. 'Q1rN…AA==', standard or URL-safe) or hex. Exactly one RSA block — 256 bytes for a 2048-bit key, 512 for 4096-bit." },
                    "private_key": { "type": "string", "description": "Your RSA private key, PEM-encoded: PKCS#8 '-----BEGIN PRIVATE KEY-----', PKCS#1 '-----BEGIN RSA PRIVATE KEY-----', or passphrase-protected '-----BEGIN ENCRYPTED PRIVATE KEY-----'." },
                    "passphrase": { "type": "string", "default": "", "description": "Passphrase that unlocks an encrypted PKCS#8 ('BEGIN ENCRYPTED PRIVATE KEY') key, e.g. 'hunter2'. Leave empty for an unencrypted key." },
                    "padding": { "type": "string", "enum": ["oaep", "pkcs1v15"], "default": "oaep", "description": "Padding the ciphertext was encrypted with: oaep (default, RSAES-OAEP) or pkcs1v15 (legacy RSAES-PKCS1-v1_5). Must match the sender exactly." },
                    "hash": { "type": "string", "enum": ["sha256", "sha384", "sha512"], "default": "sha256", "description": "OAEP digest (MGF1 + label hash) the sender used; sha256 (default), sha384, or sha512. Must match exactly. Ignored for pkcs1v15." },
                    "ciphertext_encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the ciphertext is encoded: auto (default — hex if it is all hex digits, else base64), base64, or hex." },
                    "output_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to render the recovered plaintext: utf8 (default, readable text), hex, or base64 (use these for binary payloads such as a wrapped AES key)." }
                },
                "required": ["ciphertext", "private_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
