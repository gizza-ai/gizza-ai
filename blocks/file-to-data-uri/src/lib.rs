//! gizza-ai/file-to-data-uri — encode a file as a self-contained base64 `data:`
//! URI for inline embedding (CSS `url(...)`, `<img src>`, email, etc.).
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) → take its
//! bytes + MIME → `core::to_data_uri` → flat JSON the LLM reads directly. An
//! optional `mime` argument overrides the detected content type.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report fits neither the pure-text
//! nor the ffmpeg media page shape — the F3 no-page file-input pattern, like
//! file-hash / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

// Data URIs balloon ~33% over the raw bytes and are meant for SMALL assets
// (icons, fonts, tiny images). Cap the input so the URI stays usable.
const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Optional MIME override (else the fetched/attachment content type is used).
    #[serde(default)]
    mime: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    data_uri: String,
    /// The MIME type used in the URI.
    mime: String,
    /// Size of the source file in bytes.
    bytes: usize,
    /// Length of the produced data: URI in characters.
    uri_length: usize,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. Optional `mime` overrides the
/// content type embedded in the URI.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::string("mime")
            .describe("Optional MIME type to embed (e.g. image/png). Defaults to the file's detected content type."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileToDataUri;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-to-data-uri",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode a file as a base64 data: URI",
    requires = ["wafer-run/network"],
    skill(
        description = "Encode a (small) file as a self-contained base64 data: URI — 'data:<mime>;base64,<...>' — for inline embedding in CSS url(...), an <img src>, an email, or a JSON payload. The content type is taken from the file unless you pass a mime override. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). Best for assets up to a few MB.",
        parameters = schema_json()
    ),
)]
impl FileToDataUri {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("file-to-data-uri")?;
    let (bytes, detected_mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let mime_in = match args.mime {
        Some(m) if !m.trim().is_empty() => m,
        _ => detected_mime,
    };
    let data_uri = gizza_ai_file_to_data_uri_core::to_data_uri(&bytes, &mime_in);
    let mime = gizza_ai_file_to_data_uri_core::normalize_mime(&mime_in).to_string();

    let resp = Resp {
        uri_length: data_uri.chars().count(),
        bytes: bytes.len(),
        mime,
        data_uri,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize file-to-data-uri response: {e}")))
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
                    "url":  { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mime": { "type": "string", "description": "Optional MIME type to embed (e.g. image/png). Defaults to the file's detected content type." }
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
