//! gizza-ai/file-metadata-inspect — surface every metadata block embedded in a
//! file: image EXIF/TIFF + XMP, PDF `/Info` + XMP, and Office/OpenDocument/EPUB
//! document properties.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::inspect` (pure, format sniffed from the magic bytes) → flat JSON the
//! LLM reads directly (format, grouped fields, decoded GPS, a plain-English
//! summary, and privacy notes). An unsupported format is NOT an error — it
//! comes back with a "no supported metadata found" summary.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→report tool fits neither the pure-text
//! page nor the ffmpeg file→media page shape — the no-page file-input pattern,
//! like detect-file-type / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Metadata lives in headers and small trailing blocks, but the file has to be
/// fetched whole to reach a PDF trailer or a ZIP central directory.
const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    /// Human-readable format name of the detected container, e.g. `PDF document`.
    format: String,
    /// Detected media type, e.g. `image/jpeg`.
    mime: String,
    /// Coarse bucket: image / document / archive / …
    category: String,
    /// Size of the inspected file in bytes.
    bytes: usize,
    /// The source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// Total number of metadata fields found across every group.
    field_count: usize,
    /// Metadata blocks found (`EXIF`, `XMP`, `PDF Info`, `Document properties`…).
    groups: Vec<gizza_ai_file_metadata_inspect_core::Group>,
    /// Capture coordinates decoded from EXIF GPS tags, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    gps: Option<gizza_ai_file_metadata_inspect_core::Gps>,
    /// One-line verdict — including "no supported metadata found".
    summary: String,
    /// Privacy-relevant observations and partial-failure notes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. No other parameters: the format is detected automatically
/// and every metadata block found is reported.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileMetadataInspect;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-metadata-inspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Show the hidden metadata embedded in a file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Show every metadata block embedded in a file — the hidden data you would leak by sharing it. Reads image EXIF/TIFF (camera make/model, exposure, lens, software, timestamps) with GPS decoded to decimal latitude/longitude, XMP packets (dc:title, dc:creator, xmp:CreatorTool and the rest), PDF document information (Title, Author, Subject, Keywords, Creator, Producer, creation/modification dates) plus PDF version, page count and encryption state, and Office/OpenDocument/EPUB document properties from the container (author, last modified by, revision, application, company). The format is detected from the file's magic bytes, so a wrong or missing extension does not matter. A file in an unsupported format, or one that simply carries nothing, returns a 'no supported metadata found' summary rather than an error. Pure and deterministic; the file is inspected in place and never modified. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FileMetadataInspect {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("file-metadata-inspect")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let report =
        gizza_ai_file_metadata_inspect_core::inspect(&bytes).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        format: report.format,
        mime: report.mime,
        category: report.category,
        bytes: report.bytes,
        filename: (!filename.is_empty()).then_some(filename),
        field_count: report.field_count,
        groups: report.groups,
        gps: report.gps,
        summary: report.summary,
        notes: report.notes,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize file-metadata-inspect response: {e}"))
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
