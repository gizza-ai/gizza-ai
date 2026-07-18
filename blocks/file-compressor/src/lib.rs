//! gizza-ai/file-compressor — a unified file compressor/decompressor over four
//! general-purpose codecs (gzip, xz, brotli, zstd), returned as a downloadable
//! file. One tool for both directions and every codec.
//!
//! Pipeline: resolve the source file → `core::process` (pure-Rust flate2 /
//! lzma-rust2 / brotli / ruzstd) → base64 envelope. On compress the output is
//! `<input><suffix>` (`.gz`/`.xz`/`.br`/`.zst`); on decompress the matching
//! suffix is stripped (else `.out` is appended). Decompression is bomb-guarded.
//!
//! zstd is decompress-only: the standard zstd encoder is a C library that can't
//! build to wasm here, so zstd + compress returns a clear error steering the
//! user to gzip/xz/brotli (see the competitor-analysis doc).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gzip-compress / lzma-compress).
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
    #[serde(default = "default_operation")]
    operation: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_level")]
    level: u64,
}
fn default_operation() -> String {
    "compress".to_string()
}
fn default_format() -> String {
    "gzip".to_string()
}
fn default_level() -> u64 {
    6
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `operation`, `format` and `level`
/// drive the codec. All choices are fixed enums so the LLM can only pick valid
/// values.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("operation", ["compress", "decompress"])
                .default("compress")
                .describe("Whether to compress or decompress the file (default compress)."),
        )
        .param(
            Param::enumv("format", ["gzip", "xz", "brotli", "zstd"])
                .default("gzip")
                .describe(
                    "Codec: gzip (.gz), xz (.xz/LZMA2), brotli (.br), or zstd (.zst). \
                     zstd is decompress-only; zstd + compress returns an error.",
                ),
        )
        .param(
            Param::integer("level")
                .min(1.0)
                .max(9.0)
                .default(6)
                .describe(
                    "Compression level 1-9 (1 = fastest, 9 = smallest; default 6). \
                     Ignored when decompressing.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileCompressor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-compressor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress or decompress a file (gzip, xz, brotli, zstd)",
    requires = ["wafer-run/network"],
    skill(
        description = "Compress or decompress a file with a chosen general-purpose codec, returned for download. operation is compress or decompress (default compress). format is gzip (.gz), xz (.xz / LZMA2, usually the smallest), brotli (.br), or zstd (.zst). level sets the compression level 1-9 (default 6; higher = smaller but slower; ignored when decompressing). On compress the output is named <input><suffix>; on decompress the matching suffix is stripped. zstd is decompress-only — zstd + compress returns an error (compress with gzip, xz, or brotli instead). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FileCompressor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// On compress, append the codec suffix. On decompress, strip a matching suffix
/// to recover the original name, else append `.out` so the download never
/// collides with the input.
#[cfg(target_arch = "wasm32")]
fn output_name(base: &str, op: gizza_ai_file_compressor_core::Operation, suffix: &str) -> String {
    use gizza_ai_file_compressor_core::Operation;
    match op {
        Operation::Compress => format!("{base}{suffix}"),
        Operation::Decompress => match base.strip_suffix(suffix) {
            Some(stripped) if !stripped.is_empty() => stripped.to_string(),
            _ => format!("{base}.out"),
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_file_compressor_core::{process, Format, Operation};

    let args: Args = serde_json::from_slice(&body).invalid_args("file-compressor")?;
    let op = Operation::parse(&args.operation).map_err(SkillError::InvalidArgs)?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let level = args.level.clamp(1, 9) as u32;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let out = process(op, format, &bytes, level).map_err(SkillError::InvalidArgs)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let filename = output_name(&base, op, format.suffix());
    let in_len = bytes.len();
    let out_len = out.len();

    let (mime, for_llm) = match op {
        Operation::Compress => {
            let ratio = if in_len > 0 { 100 - (out_len * 100 / in_len.max(1)) } else { 0 };
            (
                format.compressed_mime().to_string(),
                format!(
                    "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes {}, ~{ratio}% smaller, level {level})",
                    format.label()
                ),
            )
        }
        Operation::Decompress => {
            let expansion = if in_len > 0 {
                format!("{:.1}x", out_len as f64 / in_len as f64)
            } else {
                "—".to_string()
            };
            (
                "application/octet-stream".to_string(),
                format!(
                    "decompressed {base} ({in_len} bytes {}) → {filename} ({out_len} bytes original, {expansion} expansion)",
                    format.label()
                ),
            )
        }
    };

    let data_url = format!("data:{mime};base64,{}", B64.encode(&out));
    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime, filename },
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
                    "operation": { "type": "string", "enum": ["compress", "decompress"], "default": "compress", "description": "Whether to compress or decompress the file (default compress)." },
                    "format": { "type": "string", "enum": ["gzip", "xz", "brotli", "zstd"], "default": "gzip", "description": "Codec: gzip (.gz), xz (.xz/LZMA2), brotli (.br), or zstd (.zst). zstd is decompress-only; zstd + compress returns an error." },
                    "level": { "type": "integer", "minimum": 1, "maximum": 9, "default": 6, "description": "Compression level 1-9 (1 = fastest, 9 = smallest; default 6). Ignored when decompressing." }
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
