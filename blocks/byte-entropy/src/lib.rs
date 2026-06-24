//! gizza-ai/byte-entropy — measure the Shannon entropy of a file, overall and
//! per block, to spot encrypted / compressed / packed regions.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::analyze` (pure, dependency-free) → flat JSON the LLM reads directly
//! (overall entropy, distinct byte count, an assessment, and the per-block
//! entropy series with each block's offset/size).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the "no-page
//! file-input" pattern, like detect-file-type / web-fetch).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_byte_entropy_core::{DEFAULT_BLOCK_SIZE, MAX_BLOCK_SIZE, MIN_BLOCK_SIZE};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Per-block window size in bytes (clamped to [16, 4194304]).
    block_size: Option<u32>,
}

#[derive(Serialize)]
struct BlockOut {
    offset: usize,
    size: usize,
    entropy: f64,
}

#[derive(Serialize)]
struct Resp {
    /// Total number of bytes analysed.
    total_bytes: usize,
    /// Shannon entropy over the whole file, bits/byte (0.0–8.0).
    overall_entropy: f64,
    /// How many of the 256 possible byte values occur at least once.
    distinct_bytes: usize,
    /// The per-block window size used.
    block_size: usize,
    /// Lowest per-block entropy (omitted for an empty file).
    #[serde(skip_serializing_if = "Option::is_none")]
    min_block_entropy: Option<f64>,
    /// Highest per-block entropy (omitted for an empty file).
    #[serde(skip_serializing_if = "Option::is_none")]
    max_block_entropy: Option<f64>,
    /// A short plain-English classification of the overall entropy.
    assessment: String,
    /// Per-block entropy series, in order.
    blocks: Vec<BlockOut>,
    /// The source filename, when one was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

/// Round a 0–8 bits/byte entropy to 4 decimal places for stable JSON output.
fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `block_size` is an optional
/// integer for the per-block breakdown granularity.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::integer("block_size")
            .min(MIN_BLOCK_SIZE as f64)
            .max(MAX_BLOCK_SIZE as f64)
            .default(DEFAULT_BLOCK_SIZE as i64)
            .describe(
                "Window size in bytes for the per-block entropy breakdown (16–4194304, default 256). Smaller blocks reveal finer-grained high/low-entropy regions.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ByteEntropy;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/byte-entropy",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Measure a file's Shannon entropy, overall and per block",
    requires = ["wafer-run/network"],
    skill(
        description = "Compute the Shannon entropy (bits per byte, 0–8) of a file, both overall and across fixed-size blocks, to locate encrypted, compressed, or packed regions. High uniform entropy (~7.5–8.0) indicates random-looking data (encryption/compression); low entropy indicates text, code, padding, or repetitive structure. Returns total_bytes, overall_entropy, distinct_bytes (how many of 256 byte values occur), an assessment string, the min/max per-block entropy, and a blocks array (each with offset, size, entropy). Set block_size to control the per-block window (16–4194304 bytes, default 256). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ByteEntropy {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("byte-entropy")?;
    let block_size = args
        .block_size
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_BLOCK_SIZE);
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let resp = build_resp(&bytes, block_size, &filename);
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize byte-entropy response: {e}")))
}

/// Build the response from raw bytes — pure, so it's unit-testable on any target.
fn build_resp(bytes: &[u8], block_size: usize, filename: &str) -> Resp {
    let report = gizza_ai_byte_entropy_core::analyze(bytes, block_size);
    Resp {
        total_bytes: report.total_bytes,
        overall_entropy: round4(report.overall_entropy),
        distinct_bytes: report.distinct_bytes,
        block_size: report.block_size,
        min_block_entropy: report.min_block_entropy.map(round4),
        max_block_entropy: report.max_block_entropy.map(round4),
        assessment: report.assessment,
        blocks: report
            .blocks
            .into_iter()
            .map(|b| BlockOut {
                offset: b.offset,
                size: b.size,
                entropy: round4(b.entropy),
            })
            .collect(),
        filename: (!filename.is_empty()).then(|| filename.to_string()),
    }
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "block_size": {
                        "type": "integer",
                        "minimum": 16,
                        "maximum": 4194304,
                        "default": 256,
                        "description": "Window size in bytes for the per-block entropy breakdown (16–4194304, default 256). Smaller blocks reveal finer-grained high/low-entropy regions."
                    }
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
    fn resp_for_constant_buffer() {
        let r = build_resp(&[0u8; 1000], 256, "zeros.bin");
        assert_eq!(r.total_bytes, 1000);
        assert_eq!(r.overall_entropy, 0.0);
        assert_eq!(r.distinct_bytes, 1);
        assert_eq!(r.block_size, 256);
        assert_eq!(r.blocks.len(), 4);
        assert_eq!(r.filename.as_deref(), Some("zeros.bin"));
        assert!(r.assessment.starts_with("very low"));
    }

    #[test]
    fn resp_for_uniform_bytes_is_max_entropy() {
        let data: Vec<u8> = (0..=255u8).collect();
        let r = build_resp(&data, 256, "");
        assert_eq!(r.overall_entropy, 8.0);
        assert_eq!(r.distinct_bytes, 256);
        assert!(r.filename.is_none());
        assert!(r.assessment.starts_with("very high"));
    }
}
