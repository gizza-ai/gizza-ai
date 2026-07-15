//! gizza-ai/detect-file-type — identify a file's true format from its magic
//! bytes, independent of the filename or extension.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::detect` (pure, dependency-free byte sniffing) → flat JSON the LLM
//! reads directly (mime, extension, kind, category, size, and a mismatch note
//! when the filename's extension disagrees with the detected type).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the F3 "no-page
//! file-input" pattern, like web-fetch / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // only the header matters, but allow large inputs

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    /// Canonical media type, e.g. `image/png`.
    mime: String,
    /// Usual extension without the dot, e.g. `png`.
    extension: String,
    /// Human-readable format name, e.g. `PNG image`.
    kind: String,
    /// Coarse bucket: image / video / audio / document / archive / font /
    /// executable / database / text / data / unknown.
    category: String,
    /// Size of the inspected file in bytes.
    bytes: usize,
    /// The source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// Set when the filename's extension disagrees with the detected type
    /// (a renamed / mislabelled file). Omitted when they agree or no filename.
    #[serde(skip_serializing_if = "Option::is_none")]
    extension_mismatch: Option<String>,
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
struct DetectFileType;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/detect-file-type",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Identify a file's true format from its magic bytes",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Identify a file's true format by inspecting its leading bytes (magic numbers), independent of its name or extension — useful for files with a missing, wrong, or generic extension. Returns the media type (MIME), usual extension, a human-readable format name, and a coarse category (image/video/audio/document/archive/font/executable/database/text/data). Recognises images, audio, video, PDFs/Office docs, archives, fonts, executables, SQLite, and common text formats (XML/SVG/HTML/JSON). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl DetectFileType {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Lowercase extension (without dot) of a filename, if it has a plausible one.
fn filename_ext(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let (_, ext) = base.rsplit_once('.')?;
    if ext.is_empty() || ext.len() > 8 || ext.contains(' ') {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// Canonicalise an extension so well-known aliases compare equal.
fn ext_alias(e: &str) -> &str {
    match e {
        "jpeg" => "jpg",
        "tif" => "tiff",
        "htm" => "html",
        "midi" => "mid",
        "mpeg" | "mpg" => "mpg",
        // ZIP-based formats legitimately sniff as their specific OOXML type,
        // but a generic .zip name over a docx (or vice-versa) isn't "wrong".
        "docx" | "xlsx" | "pptx" | "epub" | "jar" | "apk" | "odt" | "ods" | "zip" => "zip",
        other => other,
    }
}

/// Whether a claimed extension is consistent with a detected one (treating a few
/// well-known aliases as equivalent so we don't cry wolf on jpg/jpeg etc.).
fn ext_consistent(claimed: &str, detected: &str) -> bool {
    claimed == detected || ext_alias(claimed) == ext_alias(detected)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("detect-file-type")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let det = gizza_ai_detect_file_type_core::detect(&bytes)
        .ok_or_else(|| SkillError::InvalidArgs("the file is empty — nothing to detect".into()))?;

    let filename_opt = (!filename.is_empty()).then(|| filename.clone());
    let extension_mismatch = filename_opt
        .as_deref()
        .and_then(filename_ext)
        .filter(|claimed| !ext_consistent(claimed, det.ext))
        .map(|claimed| {
            format!(
                "filename extension '.{claimed}' does not match the detected type ('.{}')",
                det.ext
            )
        });

    let resp = Resp {
        mime: det.mime.to_string(),
        extension: det.ext.to_string(),
        kind: det.kind.to_string(),
        category: det.category.to_string(),
        bytes: bytes.len(),
        filename: filename_opt,
        extension_mismatch,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize detect-file-type response: {e}")))
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

    #[test]
    fn ext_helpers() {
        assert_eq!(filename_ext("photo.JPG"), Some("jpg".to_string()));
        assert_eq!(filename_ext("/a/b/report.pdf"), Some("pdf".to_string()));
        assert_eq!(filename_ext("noext"), None);
        assert_eq!(filename_ext("a.b.tar.gz"), Some("gz".to_string()));
        assert!(ext_consistent("jpeg", "jpg"));
        assert!(ext_consistent("png", "png"));
        assert!(ext_consistent("docx", "zip"));
        assert!(!ext_consistent("png", "pdf"));
    }
}
