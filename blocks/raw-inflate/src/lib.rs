//! gizza-ai/raw-inflate — decompress a file containing headerless **raw
//! DEFLATE** (RFC 1951): no gzip and no zlib wrapper is expected, just the bare
//! deflate bit stream. The original bytes are returned as a downloadable file.
//!
//! Pipeline: resolve the source file → `core::inflate` (pure-Rust flate2) →
//! base64 envelope. The output strips a trailing `.deflate`/`.raw`/`.zz`
//! extension if present (else appends `.out`). Raw deflate carries no magic
//! bytes, no checksum and no filename header — it is the inverse of
//! `raw-deflate` / any `DeflateEncoder` output.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like raw-deflate / gunzip).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; no other params.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RawInflate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/raw-inflate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decompress a headerless raw DEFLATE file",
    requires = ["wafer-run/network"],
    skill(
        description = "Decompress a file containing headerless raw DEFLATE (RFC 1951) back to its original bytes — no gzip and no zlib wrapper is expected, just the bare deflate bit stream. This is the inverse of raw-deflate / any DeflateEncoder output. The result is returned for download with any trailing .deflate/.raw/.zz extension stripped. A gzip- or zlib-wrapped stream (or corrupt/truncated data) is rejected with an error. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl RawInflate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Strip a known raw-deflate suffix to recover the original name; else append
/// `.out` so the download never collides with the compressed input.
#[cfg(target_arch = "wasm32")]
fn output_name(input: &str) -> String {
    let base = if input.is_empty() { "file" } else { input };
    for suf in [".deflate", ".raw", ".zz"] {
        if let Some(stripped) = base.strip_suffix(suf) {
            if !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    format!("{base}.out")
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("raw-inflate")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let original = gizza_ai_raw_inflate_core::inflate(&bytes).map_err(SkillError::InvalidArgs)?;

    let filename = output_name(&in_filename);
    let in_len = bytes.len();
    let out_len = original.len();
    let ratio = if in_len > 0 {
        format!("{:.1}x", out_len as f64 / in_len as f64)
    } else {
        "—".to_string()
    };
    let data_url = format!("data:application/octet-stream;base64,{}", B64.encode(&original));
    let in_label = if in_filename.is_empty() { "input" } else { in_filename.as_str() };
    let for_llm = format!(
        "inflated {in_label} ({in_len} bytes raw DEFLATE) → {filename} ({out_len} bytes original, {ratio} expansion)"
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/octet-stream".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
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
