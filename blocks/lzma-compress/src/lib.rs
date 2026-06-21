//! gizza-ai/lzma-compress — compress a file (or any bytes) into LZMA `.xz`,
//! returned as a downloadable file. The inverse of an xz/lzma decompressor.
//!
//! Pipeline: resolve the source file → `core::xz_compress` (pure-Rust
//! `lzma-rust2`, the tukaani xz-for-java port) → base64 envelope. The output is
//! named `<input>.xz` and is a canonical `.xz` stream (validates with `xz -t`).
//!
//! `.xz` (LZMA2) usually beats gzip/zip on ratio at the cost of speed — pick it
//! when smallest size matters. Pure Rust → runs on ALL backends including the
//! chat Service Worker. Surfaces: chat + CLI. No standalone page (file→file
//! output, the no-page file-input pattern, like gzip-compress / gunzip).
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

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `level` is the LZMA preset.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::integer("level")
            .min(0.0)
            .max(9.0)
            .default(6)
            .describe("LZMA preset 0-9 (0 = fastest, 9 = smallest; default 6). Higher uses more time and memory."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LzmaCompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lzma-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress a file into LZMA (.xz)",
    requires = ["wafer-run/network"],
    skill(
        description = "Compress a file (or any bytes) into an LZMA .xz file, returned for download. .xz (LZMA2) usually achieves a smaller size than gzip or zip at the cost of speed — use it when maximum compression ratio matters. level sets the LZMA preset 0-9 (default 6; higher = smaller but slower and more memory). The output is a standard .xz stream named <input>.xz that any xz / 7-Zip tool can decompress. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl LzmaCompress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("lzma-compress")?;
    let level = args.level.clamp(0, 9) as u32;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let xz = gizza_ai_lzma_compress_core::xz_compress(&bytes, level)
        .map_err(SkillError::InvalidArgs)?;

    let filename = format!("{base}.xz");
    let in_len = bytes.len();
    let out_len = xz.len();
    let ratio = if in_len > 0 { 100 - (out_len * 100 / in_len.max(1)) } else { 0 };
    let data_url = format!("data:application/x-xz;base64,{}", B64.encode(&xz));
    let for_llm = format!(
        "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes .xz/LZMA, ~{ratio}% smaller, preset {level})"
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/x-xz".to_string(), filename },
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
                    "level": { "type": "integer", "minimum": 0, "maximum": 9, "default": 6, "description": "LZMA preset 0-9 (0 = fastest, 9 = smallest; default 6). Higher uses more time and memory." }
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
