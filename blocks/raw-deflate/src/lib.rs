//! gizza-ai/raw-deflate — compress a file (or any bytes) with headerless **raw
//! DEFLATE** (RFC 1951): no gzip and no zlib wrapper, just the bare deflate bit
//! stream. Returned as a downloadable file.
//!
//! Pipeline: resolve the source file → `core::deflate` (pure-Rust flate2) →
//! base64 envelope. The output is named `<input>.deflate`. Raw deflate carries
//! no magic bytes, no checksum and no filename header — it's the smallest of the
//! three flate framings (gzip / zlib / raw) and is what other formats embed.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gzip-compress / gunzip).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_level")]
    level: u64,
}
fn default_level() -> u64 {
    6
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `level` is the deflate level.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::integer("level")
            .min(1.0)
            .max(9.0)
            .default(6)
            .describe("Compression level 1-9 (1 = fastest, 9 = smallest; default 6)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RawDeflate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/raw-deflate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress a file with headerless raw DEFLATE",
    requires = ["wafer-run/network"],
    skill(
        description = "Compress a file (or any bytes) with headerless raw DEFLATE (RFC 1951) — no gzip and no zlib wrapper, just the bare deflate bit stream, returned for download as <input>.deflate. Raw deflate has no magic bytes, no checksum and no filename header, so it is smaller than gzip/zlib and is what other formats embed. level sets the deflate level 1-9 (default 6). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl RawDeflate {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("raw-deflate")?;
    let level = args.level.clamp(1, 9) as u32;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let raw = gizza_ai_raw_deflate_core::deflate(&bytes, level).map_err(SkillError::InvalidArgs)?;

    let filename = format!("{base}.deflate");
    let in_len = bytes.len();
    let out_len = raw.len();
    // Signed % size change: positive = smaller, negative = larger (tiny or
    // already-compressed inputs can grow due to DEFLATE block overhead).
    let pct = if in_len > 0 {
        100 - (out_len as i64 * 100 / in_len as i64)
    } else {
        0
    };
    let size_note = if pct >= 0 {
        format!("~{pct}% smaller")
    } else {
        format!("~{}% larger (input too small/incompressible)", -pct)
    };
    let data_url = format!("data:application/octet-stream;base64,{}", B64.encode(&raw));
    let for_llm = format!(
        "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes raw DEFLATE, no header, {size_note}, level {level})"
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
                    "url":   { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":   { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "level": { "type": "integer", "minimum": 1, "maximum": 9, "default": 6, "description": "Compression level 1-9 (1 = fastest, 9 = smallest; default 6)." }
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
