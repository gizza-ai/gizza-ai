//! gizza-ai/encoded-payload-decoder — find and decode base64 / hex tokens and
//! gzip / zlib compressed streams embedded anywhere in a file, unwrap nested
//! layers, and surface hidden readable strings + a detected file type for each
//! binary payload.
//!
//! Pipeline: resolve the source file (any bytes) → `core::scan` → flat JSON the
//! LLM reads directly. Pure Rust → runs on ALL backends including the chat
//! Service Worker. Surfaces: chat + CLI. No standalone page (file→JSON report —
//! the F3 no-page file-input pattern, like `strings` / `detect-file-type`).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_encoded_payload_decoder_core::{scan, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_min_len")]
    min_len: u32,
    #[serde(default = "default_max_depth")]
    max_depth: u32,
}
fn default_min_len() -> u32 {
    20
}
fn default_max_depth() -> u32 {
    3
}

#[derive(Serialize)]
struct Resp {
    findings: Vec<gizza_ai_encoded_payload_decoder_core::Finding>,
    count: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::integer("min_len").min(4.0).max(4096.0).describe(
                "Minimum length of a base64/hex token run to treat as a candidate (default 20). Lower finds shorter blobs but more noise.",
            ),
        )
        .param(
            Param::integer("max_depth").min(1.0).max(6.0).describe(
                "How many nested encoding layers to unwrap, e.g. base64 → gzip → text (default 3).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EncodedPayloadDecoder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/encoded-payload-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and decode base64, hex, and gzip/zlib payloads hidden in a file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a file for base64 and hex tokens and gzip/zlib compressed streams embedded anywhere in its bytes, decode/decompress each, and unwrap nested layers (e.g. base64 of gzip of text) up to max_depth (default 3). min_len (default 20) sets the shortest base64/hex run to consider. Each finding reports its encoding, byte offset, nesting depth, and decoded content — printable payloads as text, binaries with a detected file type (e.g. image/png), a hex preview, and surfaced readable strings. Random alphanumeric noise that decodes to nothing meaningful is ignored. Provide the file as either url (HTTP/HTTPS) or ref. Runs locally — the file never leaves the device.",
        parameters = schema_json()
    ),
)]
impl EncodedPayloadDecoder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("encoded-payload-decoder")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let report = scan(
        &bytes,
        Options { min_len: args.min_len as usize, max_depth: args.max_depth as usize },
    );
    let resp = Resp {
        findings: report.findings,
        count: report.count,
        truncated: report.truncated,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize encoded-payload-decoder response: {e}"))
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
                    "url":       { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "min_len":   { "type": "integer", "minimum": 4, "maximum": 4096, "description": "Minimum length of a base64/hex token run to treat as a candidate (default 20). Lower finds shorter blobs but more noise." },
                    "max_depth": { "type": "integer", "minimum": 1, "maximum": 6, "description": "How many nested encoding layers to unwrap, e.g. base64 → gzip → text (default 3)." }
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
