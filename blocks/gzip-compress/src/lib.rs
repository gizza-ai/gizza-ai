//! gizza-ai/gzip-compress — compress a file (or any bytes) into gzip (.gz),
//! returned as a downloadable file. The inverse of gunzip.
//!
//! Pipeline: resolve the source file → `core::gzip` (pure-Rust flate2) → base64
//! envelope. The original filename is stored in the gzip header and the output
//! is named `<input>.gz`.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gunzip).
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
struct GzipCompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gzip-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress a file into gzip (.gz)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compress a file (or any bytes) into a gzip (.gz) file, returned for download. level sets the deflate level 1-9 (default 6). The original filename is recorded in the gzip header and the output is named <input>.gz. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). Decompress again with gunzip.",
        parameters = schema_json()
    ),
)]
impl GzipCompress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("gzip-compress")?;
    let level = args.level.clamp(1, 9) as u32;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let header_name = if base == "file" { None } else { Some(base.as_str()) };
    let gz = gizza_ai_gzip_compress_core::gzip(&bytes, header_name, level)
        .map_err(SkillError::InvalidArgs)?;

    let filename = format!("{base}.gz");
    let in_len = bytes.len();
    let out_len = gz.len();
    let ratio = if in_len > 0 { 100 - (out_len * 100 / in_len.max(1)) } else { 0 };
    let data_url = format!("data:application/gzip;base64,{}", B64.encode(&gz));
    let for_llm = format!(
        "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes gzip, ~{ratio}% smaller, level {level})"
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/gzip".to_string(), filename },
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
