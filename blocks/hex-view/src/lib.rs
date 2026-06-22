//! gizza-ai/hex-view — render any file as a classic hex dump (offset, hex bytes,
//! ASCII gutter).
//!
//! Pipeline: resolve the source file → `core::hex_dump` (pure, dependency-free)
//! → flat JSON the LLM reads directly. The dump is capped so a huge file doesn't
//! blow up the response.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file input + text output, the no-page
//! file-input pattern, like detect-file-type / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Cap how many bytes we dump (keeps the text response bounded).
const DEFAULT_MAX_DUMP: usize = 4096;
const HARD_MAX_DUMP: usize = 262_144; // 256 KiB of bytes dumped

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_width")]
    bytes_per_line: u64,
    #[serde(default)]
    max_bytes: Option<u64>,
}
fn default_width() -> u64 {
    16
}

#[derive(Serialize)]
struct Resp {
    dump: String,
    /// Total size of the file in bytes.
    total_bytes: usize,
    /// Number of bytes actually shown in the dump.
    shown_bytes: usize,
    /// True when the file was longer than the dump cap.
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::integer("bytes_per_line")
                .min(1.0)
                .max(64.0)
                .default(16)
                .describe("Bytes shown per row (1-64, default 16)."),
        )
        .param(
            Param::integer("max_bytes")
                .min(1.0)
                .describe("Maximum number of bytes to dump (default 4096, hard cap 262144). Use to view more or less of a large file."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HexView;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hex-view",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a file as a hex dump",
    requires = ["wafer-run/network"],
    skill(
        description = "Render any file as a classic hex dump — an 8-digit hex offset column, the bytes in hex (grouped 8+8), and an ASCII gutter (like `xxd`). bytes_per_line (1-64, default 16) sets the row width; max_bytes (default 4096, hard cap 262144) limits how much is shown. Returns the dump text plus total/shown byte counts. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl HexView {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("hex-view")?;
    let per = args.bytes_per_line.clamp(1, 64) as usize;
    let cap = args
        .max_bytes
        .map(|m| (m as usize).min(HARD_MAX_DUMP).max(1))
        .unwrap_or(DEFAULT_MAX_DUMP);

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let total = bytes.len();
    let shown = total.min(cap);
    let truncated = total > shown;

    let dump = gizza_ai_hex_view_core::hex_dump(&bytes[..shown], per);

    let resp = Resp { dump, total_bytes: total, shown_bytes: shown, truncated };
    serde_json::to_vec(&resp).map_err(|e| SkillError::Serialize(format!("serialize hex-view response: {e}")))
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
                    "url":            { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":            { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "bytes_per_line": { "type": "integer", "minimum": 1, "maximum": 64, "default": 16, "description": "Bytes shown per row (1-64, default 16)." },
                    "max_bytes":      { "type": "integer", "minimum": 1, "description": "Maximum number of bytes to dump (default 4096, hard cap 262144). Use to view more or less of a large file." }
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
