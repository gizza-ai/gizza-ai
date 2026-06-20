//! gizza-ai/file-hash — compute cryptographic checksums (MD5, SHA-1, SHA-256,
//! SHA-512, CRC-32) of any file.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::hash_all` (pure RustCrypto) → flat JSON the LLM reads directly. Useful
//! for integrity checks, deduplication, and threat-intel hash lookups.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→digests report fits neither the
//! pure-text nor the ffmpeg media page shape — the F3 no-page file-input pattern,
//! like detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Serialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(serde::Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    md5: String,
    sha1: String,
    sha256: String,
    sha512: String,
    crc32: String,
    /// Size of the hashed file in bytes.
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. No other parameters: every digest is computed.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileHash;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute MD5/SHA-1/SHA-256/SHA-512/CRC-32 checksums of a file",
    requires = ["wafer-run/network"],
    skill(
        description = "Compute cryptographic checksums of a file — MD5, SHA-1, SHA-256, SHA-512, and CRC-32 — as lowercase hex. Useful for verifying file integrity/downloads, deduplication, and threat-intel hash lookups (e.g. VirusTotal). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FileHash {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("file-hash")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let h = gizza_ai_file_hash_core::hash_all(&bytes);
    let resp = Resp {
        md5: h.md5,
        sha1: h.sha1,
        sha256: h.sha256,
        sha512: h.sha512,
        crc32: h.crc32,
        bytes: bytes.len(),
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize file-hash response: {e}")))
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
