//! gizza-ai/zstd-decompress — chat skill block on the shared tool abstraction.
//! Decodes a Zstandard-compressed (RFC 8878) payload pasted as Base64 or hex
//! back to its original bytes, rendered as text, hex, or Base64, with optional
//! size stats and a per-frame structural report. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. No host calls — runs entirely inside the
//! WASM sandbox, so it works on every backend including the chat Service Worker.
//!
//! Sibling split: `file-compressor` handles a real `.zst` FILE (url/ref →
//! download); this block is the inline/readable half for pasted payloads.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    stats: bool,
    #[serde(default)]
    frame_info: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The Zstandard-compressed payload, encoded per 'encoding' — e.g. a Base64 copy of an HTTP body served with content-encoding: zstd, a Kafka or ClickHouse record, or a hex dump of a .zst blob. ASCII whitespace and line breaks are ignored, so a wrapped paste works. Max 8 MiB compressed."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the payload in 'data' is encoded: 'auto' (default — decodes the paste as both hex and Base64 and keeps whichever yields the zstd magic number 28 b5 2f fd), 'base64' (standard or URL-safe, padding optional), or 'hex' (an optional 0x prefix is ignored)."),
        )
        .param(
            Param::enumv("output", ["text", "hex", "base64"])
                .default("text")
                .describe("How to render the decompressed bytes: 'text' (default, UTF-8 — errors if the result is binary), 'hex' (lowercase, binary-safe), or 'base64'."),
        )
        .param(
            Param::boolean("stats")
                .default(false)
                .describe("Prepend a size summary — compressed bytes, decompressed bytes, the decompressed/compressed ratio, the percentage of space saved, and how many data and skippable frames the stream held — before the payload. Default false returns only the payload."),
        )
        .param(
            Param::boolean("frame_info")
                .default(false)
                .describe("Prepend a per-frame structural report: for each data frame its compressed and decompressed size, the decoder window size, the content size the encoder declared (or that it declared none), the dictionary ID, and whether the trailing xxHash-32 content checksum was present and verified; skippable frames are listed with their magic number and payload size. Default false returns only the payload."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/zstd-decompress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a Zstandard (.zst) payload from Base64 or hex to text, hex, or Base64.",
    skill(
        description = "Decompress a Zstandard (RFC 8878, .zst) payload — the codec behind HTTP content-encoding: zstd, Kafka and ClickHouse records, and modern package and log archives — and show what is inside it. Paste the compressed blob in 'data' as Base64 or hex; encoding='auto' (default) decodes both ways and keeps whichever starts with the zstd magic number 28 b5 2f fd. Because zstd has that magic number, a wrong-codec blob is named before any decode is attempted: gzip, zlib, xz, raw LZMA, LZ4, bzip2, ZIP, 7-Zip, and tar inputs are reported along with the sibling tool that handles them. Choose output='text' (default, UTF-8), 'hex', or 'base64'; stats=true adds sizes, the compression ratio, and frame counts; frame_info=true adds a per-frame report of window size, declared content size, dictionary ID, and xxHash-32 content-checksum verification. Concatenated multi-frame streams decode in full instead of stopping after the first frame, skippable metadata frames are stepped over, and a checksum mismatch is a hard error rather than a silent pass. Dictionary-compressed frames cannot be decoded without the dictionary and error naming their Dictionary_ID. Limits: 8 MiB compressed in, 16 MiB decompressed out. For a .zst FILE by URL or ref, use file-compressor with operation=decompress format=zstd instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "zstd-decompress", |a: Args| {
            gizza_ai_zstd_decompress_core::run(
                &a.data,
                &a.encoding,
                &a.output,
                a.stats,
                a.frame_info,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The Zstandard-compressed payload, encoded per 'encoding' — e.g. a Base64 copy of an HTTP body served with content-encoding: zstd, a Kafka or ClickHouse record, or a hex dump of a .zst blob. ASCII whitespace and line breaks are ignored, so a wrapped paste works. Max 8 MiB compressed." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the payload in 'data' is encoded: 'auto' (default — decodes the paste as both hex and Base64 and keeps whichever yields the zstd magic number 28 b5 2f fd), 'base64' (standard or URL-safe, padding optional), or 'hex' (an optional 0x prefix is ignored)." },
                    "output": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to render the decompressed bytes: 'text' (default, UTF-8 — errors if the result is binary), 'hex' (lowercase, binary-safe), or 'base64'." },
                    "stats": { "type": "boolean", "default": false, "description": "Prepend a size summary — compressed bytes, decompressed bytes, the decompressed/compressed ratio, the percentage of space saved, and how many data and skippable frames the stream held — before the payload. Default false returns only the payload." },
                    "frame_info": { "type": "boolean", "default": false, "description": "Prepend a per-frame structural report: for each data frame its compressed and decompressed size, the decoder window size, the content size the encoder declared (or that it declared none), the dictionary ID, and whether the trailing xxHash-32 content checksum was present and verified; skippable frames are listed with their magic number and payload size. Default false returns only the payload." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
