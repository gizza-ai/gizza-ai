//! gizza-ai/brotli-decompress — chat skill block on the shared tool abstraction.
//! Decodes a Brotli-compressed (RFC 7932) payload pasted as Base64 or hex back
//! to its original bytes, rendered as text, hex, or Base64. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. No host calls — runs entirely inside the
//! WASM sandbox, so it works on every backend including the chat Service Worker.
//!
//! Sibling split: `file-compressor` handles a real `.br` FILE (url/ref → download);
//! this block is the inline/readable half for pasted payloads.
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
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The Brotli-compressed payload, encoded per 'encoding' — e.g. a Base64 copy of an HTTP body served with content-encoding: br, an asset-bundle chunk, or a hex dump of a .br file. ASCII whitespace and line breaks are ignored, so a wrapped paste works. Max 8 MiB compressed."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the payload in 'data' is encoded: 'auto' (default — tries hex then Base64 and keeps whichever actually Brotli-decompresses), 'base64' (standard or URL-safe, padding optional), or 'hex' (an optional 0x prefix is ignored)."),
        )
        .param(
            Param::enumv("output", ["text", "hex", "base64"])
                .default("text")
                .describe("How to render the decompressed bytes: 'text' (default, UTF-8 — errors if the result is binary), 'hex' (lowercase, binary-safe), or 'base64'."),
        )
        .param(
            Param::boolean("stats")
                .default(false)
                .describe("Prepend a size summary — compressed bytes, decompressed bytes, the decompressed/compressed ratio, and the percentage of space saved — before the payload. Default false returns only the payload."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/brotli-decompress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a Brotli (.br) payload from Base64 or hex to text, hex, or Base64.",
    skill(
        description = "Decompress a Brotli (RFC 7932) payload — the codec behind HTTP content-encoding: br and most modern web asset bundles — and show what is inside it. Paste the compressed blob in 'data' as Base64 or hex; encoding='auto' (default) works out which by decoding both and keeping whichever actually inflates. Choose output='text' (default, UTF-8), 'hex', or 'base64', and set stats=true for compressed/decompressed sizes plus the compression ratio. Brotli has no magic number, so a wrong-codec blob is diagnosed after the decode attempt: gzip, zlib, xz, LZ4, zstd, bzip2, ZIP, 7-Zip, and tar inputs are named along with the sibling tool that handles them. Limits: 8 MiB compressed in, 16 MiB decompressed out. For a .br FILE by URL or ref, use file-compressor with operation=decompress format=brotli instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "brotli-decompress", |a: Args| {
            gizza_ai_brotli_decompress_core::run(&a.data, &a.encoding, &a.output, a.stats)
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
                    "data": { "type": "string", "description": "The Brotli-compressed payload, encoded per 'encoding' — e.g. a Base64 copy of an HTTP body served with content-encoding: br, an asset-bundle chunk, or a hex dump of a .br file. ASCII whitespace and line breaks are ignored, so a wrapped paste works. Max 8 MiB compressed." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the payload in 'data' is encoded: 'auto' (default — tries hex then Base64 and keeps whichever actually Brotli-decompresses), 'base64' (standard or URL-safe, padding optional), or 'hex' (an optional 0x prefix is ignored)." },
                    "output": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to render the decompressed bytes: 'text' (default, UTF-8 — errors if the result is binary), 'hex' (lowercase, binary-safe), or 'base64'." },
                    "stats": { "type": "boolean", "default": false, "description": "Prepend a size summary — compressed bytes, decompressed bytes, the decompressed/compressed ratio, and the percentage of space saved — before the payload. Default false returns only the payload." }
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
