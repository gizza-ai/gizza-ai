//! gizza-ai/encrypt-file — passphrase-encrypt or decrypt a file (AES-256-GCM).
//!
//! Pure-Rust crypto (aes-gcm + PBKDF2), so like the other pure tools it runs on
//! ALL backends including the chat Service Worker. Pipeline: resolve the source
//! file (URL fetch or attachment ref, any bytes) → `core::encrypt`/`decrypt` →
//! base64 envelope (the encrypted/decrypted bytes as a downloadable file).
//! Surfaces: chat + CLI. No standalone page (a pure-Rust file-bytes output has
//! no page mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    passphrase: String,
}
fn default_mode() -> String { "encrypt".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("mode", ["encrypt", "decrypt"])
                .default("encrypt")
                .describe("Whether to encrypt the file (default) or decrypt a blob produced by this tool."),
        )
        .param(
            Param::string("passphrase")
                .required()
                .describe("The passphrase. The same passphrase is required to decrypt."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EncryptFile;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/encrypt-file",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt a file with a passphrase (AES-256-GCM)",
    requires = ["wafer-run/network"],
    skill(
        description = "Encrypt a file with a passphrase using AES-256-GCM (key derived via PBKDF2-HMAC-SHA256), producing a portable encrypted blob; or decrypt a blob this tool produced. Set mode='encrypt' (default) or 'decrypt' and provide the passphrase. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl EncryptFile {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("encrypt-file")?;
    if args.passphrase.is_empty() {
        return Err(SkillError::InvalidArgs("passphrase is required".into()));
    }
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (out, filename, verb) = match args.mode.as_str() {
        "" | "encrypt" => {
            let blob = gizza_ai_encrypt_file_core::encrypt(&bytes, &args.passphrase)
                .map_err(SkillError::InvalidArgs)?;
            (blob, format!("{in_filename}.enc"), "encrypted")
        }
        "decrypt" => {
            let plain = gizza_ai_encrypt_file_core::decrypt(&bytes, &args.passphrase)
                .map_err(SkillError::InvalidArgs)?;
            let name = in_filename.strip_suffix(".enc").map(|s| s.to_string())
                .unwrap_or_else(|| format!("{in_filename}.dec"));
            (plain, name, "decrypted")
        }
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "invalid mode '{other}' (use encrypt or decrypt)"
            )))
        }
    };
    let out_len = out.len();
    let encoded = B64.encode(&out);
    let data_url = format!("data:application/octet-stream;base64,{encoded}");

    let env = Envelope {
        for_llm: format!("{verb} {in_filename} ({out_len}-byte file: {filename})"),
        for_ui: ForUi { data_url, mime: "application/octet-stream".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::File url⊕ref oneOf + mode enum + required passphrase).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":       { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "Whether to encrypt the file (default) or decrypt a blob produced by this tool." },
                    "passphrase": { "type": "string", "description": "The passphrase. The same passphrase is required to decrypt." }
                },
                "required": ["passphrase"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
