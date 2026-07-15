//! gizza-ai/zip-inspect — list a .zip archive's contents (entry names, sizes,
//! compression method, CRC-32) WITHOUT extracting or decompressing anything.
//!
//! Pipeline: resolve the source file → `core::inspect` (pure-Rust zip, reads only
//! the central directory) → flat JSON the LLM reads directly (each entry with its
//! name, uncompressed/compressed size, compression method, CRC, ratio, dir/encrypted
//! flags, and modification time, plus archive-wide totals).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→JSON, the no-page file-input pattern, like
//! unzip / extract-tar / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_zip_inspect_core::inspect;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    entries: Vec<gizza_ai_zip_inspect_core::Entry>,
    count: usize,
    file_count: usize,
    dir_count: usize,
    total_size: u64,
    total_compressed_size: u64,
    total_ratio_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ZipInspect;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/zip-inspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List the contents of a .zip archive without extracting",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "List the contents of a .zip archive WITHOUT extracting anything. Reads only the archive's central directory and returns every entry with its path, uncompressed size, compressed size, compression method (e.g. Stored, Deflated, Bzip2), CRC-32 checksum, and the percentage saved by compression — plus whether the entry is a directory or encrypted and its modification time when recorded. Also reports archive-wide totals (entry count, file/directory counts, total uncompressed and compressed size, overall compression ratio). Nothing is decompressed, so it works on any zip including ones using methods this tool can't extract. Provide the zip as either url (HTTP/HTTPS) or ref. Runs locally — the archive never leaves the device.",
        parameters = schema_json()
    ),
)]
impl ZipInspect {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("zip-inspect")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let listing = inspect(&bytes).map_err(SkillError::InvalidArgs)?;
    let resp = Resp {
        entries: listing.entries,
        count: listing.count,
        file_count: listing.file_count,
        dir_count: listing.dir_count,
        total_size: listing.total_size,
        total_compressed_size: listing.total_compressed_size,
        total_ratio_pct: listing.total_ratio_pct,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize zip-inspect response: {e}")))
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
