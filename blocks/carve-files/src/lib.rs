//! gizza-ai/carve-files — recover embedded files from a binary blob by scanning
//! for known magic-byte signatures ("file carving").
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::carve` (pure-Rust signature scan) → flat JSON the LLM reads directly
//! (each recovered file with its byte offset, size, detected type, and inline
//! base64 content within a budget).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (binary file→JSON, the no-page file-input
//! pattern, like unzip / extract-tar / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_carve_files_core::{carve, DEFAULT_CONTENT_BUDGET};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    files: Vec<gizza_ai_carve_files_core::Carved>,
    count: usize,
    scanned_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a blob arrives via URL fetch or
/// an attachment ref. No other parameters: carving is fully automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CarveFiles;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/carve-files",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recover embedded files from a binary blob by magic-byte signatures",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a binary blob (disk image, memory dump, corrupted container, or any concatenation of files) for embedded files by their magic-byte signatures and extract the recovered files — this is 'file carving'. Returns each recovered file with its byte offset, size, detected type (extension / format name / MIME), whether its end was found exactly, and its content inline as base64 (up to an internal budget; larger files are listed without content). Recognises PNG, JPEG, GIF, BMP, PDF, ZIP, gzip, bzip2, XZ, 7-Zip, RAR, Ogg, FLAC, MP3, RIFF (WAV/AVI/WEBP), and SQLite. Provide the blob as either url (HTTP/HTTPS) or ref (id from a prior tool call). Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl CarveFiles {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("carve-files")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    if bytes.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the blob is empty — nothing to carve".into(),
        ));
    }
    let result = carve(&bytes, DEFAULT_CONTENT_BUDGET);
    let resp = Resp {
        files: result.files,
        count: result.count,
        scanned_bytes: result.scanned_bytes,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize carve-files response: {e}")))
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
