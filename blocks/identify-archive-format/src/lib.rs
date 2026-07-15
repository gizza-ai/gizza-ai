//! gizza-ai/identify-archive-format — detect the COMPRESSION or ARCHIVE format
//! of an uploaded blob from its magic bytes, so you know which decompressor /
//! unpacker to reach for.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::detect` (pure, dependency-free magic-byte sniffing) → flat JSON the
//! LLM reads directly (format id, name, mime, extension, archive-vs-compressor
//! kind, a concrete decompression hint, and the inspected size).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the no-page
//! file-input pattern, like detect-file-type / web-fetch / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // only the header matters; allow large inputs

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    /// Short format id, e.g. `gzip`, `zip`, `xz`, `zstd`.
    format: String,
    /// Human-readable format name, e.g. `Gzip compressed stream`.
    name: String,
    /// Canonical media type, e.g. `application/gzip`.
    mime: String,
    /// Usual extension without the dot, e.g. `gz`.
    extension: String,
    /// `archive` (holds multiple files) or `compressor` (single byte stream).
    kind: String,
    /// A concrete command/tool to decompress or unpack it.
    decompress_with: String,
    /// Size of the inspected file in bytes.
    bytes: usize,
    /// The source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. No other parameters: detection is fully automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IdentifyArchiveFormat;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/identify-archive-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect the compression or archive format of a blob from its magic bytes",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect the COMPRESSION or ARCHIVE format of an uploaded file/blob by inspecting its leading bytes (magic numbers), independent of its name or extension — answers 'what compression format is this and which decompressor do I use?'. Returns a short format id, a human-readable name, the media type (MIME), the usual extension, whether it is a single-stream compressor or a multi-file archive, and a concrete decompression command. Recognises gzip, bzip2, xz, raw LZMA, zstd, lz4, lzip, lzop, zlib (deflate), Unix compress (.Z), Snappy framing, and the ZIP, TAR, 7-Zip, RAR, ar/.deb, cpio, and Microsoft Cabinet container archives. Reports a clear error when the blob is not a recognised archive/compression format. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl IdentifyArchiveFormat {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("identify-archive-format")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let det = gizza_ai_identify_archive_format_core::detect(&bytes).ok_or_else(|| {
        if bytes.is_empty() {
            SkillError::InvalidArgs("the file is empty — nothing to detect".into())
        } else {
            SkillError::InvalidArgs(
                "not a recognised archive or compression format (the leading bytes do not match \
                 any known archive/compressor magic — it may be an ordinary uncompressed file)"
                    .into(),
            )
        }
    })?;

    let filename_opt = (!filename.is_empty()).then(|| filename.clone());

    let resp = Resp {
        format: det.format.to_string(),
        name: det.name.to_string(),
        mime: det.mime.to_string(),
        extension: det.ext.to_string(),
        kind: det.kind.to_string(),
        decompress_with: det.decompress_with.to_string(),
        bytes: bytes.len(),
        filename: filename_opt,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize identify-archive-format response: {e}"))
    })
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
