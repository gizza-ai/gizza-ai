//! gizza-ai/scan-embedded-files — find file signatures (magic numbers) anywhere
//! inside a file, not just at offset 0, to reveal hidden / appended / embedded
//! payloads (a ZIP appended to a PNG, an EXE carved into a JPEG, a PDF embedded
//! in an image — "polyglot" files and steganographic carriers).
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::scan` (pure, dependency-free byte scanning) → flat JSON the LLM reads
//! directly (size, the leading/declared type, the list of every signature found
//! with its byte offset, and a flag for payloads appended past the host file's
//! logical end).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the F3 "no-page
//! file-input" pattern, like detect-file-type / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct HitOut {
    /// Byte offset where the signature begins.
    offset: usize,
    /// Usual extension without the dot, e.g. `zip`.
    extension: String,
    /// Human-readable format name, e.g. `ZIP local-file entry`.
    kind: String,
    /// Canonical media type, e.g. `application/zip`.
    mime: String,
    /// True for the signature at offset 0 (the file's own / declared type).
    is_leading: bool,
    /// True when this signature begins at/after the leading file's logical end,
    /// i.e. data appended past the host format (a strong hidden-payload signal).
    appended: bool,
}

#[derive(Serialize)]
struct Resp {
    /// Size of the inspected file in bytes.
    bytes: usize,
    /// The leading (declared) type at offset 0, when recognised.
    #[serde(skip_serializing_if = "Option::is_none")]
    leading_type: Option<String>,
    /// Number of signatures found that are NOT the leading file itself.
    embedded_count: usize,
    /// True when any payload was found appended past the host file's logical end.
    has_appended_payload: bool,
    /// Every signature occurrence found, in ascending offset order.
    hits: Vec<HitOut>,
    /// The source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. No other parameters: scanning is fully automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ScanEmbeddedFiles;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/scan-embedded-files",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan a file for embedded or appended file signatures",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a file for embedded file signatures (magic numbers) appearing anywhere inside it, not just at the start — revealing hidden, appended, or concatenated files such as a ZIP appended to a PNG, an executable carved into a JPEG, or a PDF embedded in an image (polyglot files and steganographic carriers). Returns the file size, the leading (declared) type, and every signature found with its byte offset, format name, media type, and whether it was appended past the host file's logical end. Recognises images, audio, video, PDFs/Office, archives (ZIP/RAR/7z/gz/xz/zstd), fonts, executables (ELF/Mach-O/Wasm/Java), and SQLite. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ScanEmbeddedFiles {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("scan-embedded-files")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    if bytes.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the file is empty — nothing to scan".into(),
        ));
    }

    let scan = gizza_ai_scan_embedded_files_core::scan(&bytes);

    let leading_type = scan
        .hits
        .iter()
        .find(|h| h.is_leading)
        .map(|h| h.kind.clone());
    let embedded_count = scan.hits.iter().filter(|h| !h.is_leading).count();
    let has_appended_payload = scan.hits.iter().any(|h| h.appended);

    let hits: Vec<HitOut> = scan
        .hits
        .into_iter()
        .map(|h| HitOut {
            offset: h.offset,
            extension: h.ext,
            kind: h.kind,
            mime: h.mime,
            is_leading: h.is_leading,
            appended: h.appended,
        })
        .collect();

    let resp = Resp {
        bytes: bytes.len(),
        leading_type,
        embedded_count,
        has_appended_payload,
        hits,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize scan-embedded-files response: {e}")))
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
