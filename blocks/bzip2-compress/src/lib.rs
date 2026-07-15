//! gizza-ai/bzip2-compress — compress a file (or any bytes) into bzip2 (.bz2),
//! returned as a downloadable file. bzip2 uses the Burrows–Wheeler transform and
//! usually beats gzip on text (at the cost of speed). The inverse is any standard
//! `bunzip2` / `bzip2 -d`.
//!
//! Pipeline: resolve the source file → `core::bzip2` (pure-Rust banzai) → base64
//! envelope. The output is named `<input>.bz2`.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gzip-compress).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_level")]
    level: u64,
}
fn default_level() -> u64 {
    9
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `level` is the bzip2 block size.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::integer("level")
            .min(1.0)
            .max(9.0)
            .default(9)
            .describe("Block size 1-9 — each unit is a 100 KB BWT block (1 = least memory, 9 = best ratio; default 9)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Bzip2Compress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bzip2-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress a file into bzip2 (.bz2)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compress a file (or any bytes) into a bzip2 (.bz2) file, returned for download. bzip2 uses the Burrows–Wheeler transform and usually compresses text smaller than gzip (though slower). level sets the block size 1-9 (each unit is a 100 KB block; 9 = best ratio, default 9). The output is named <input>.bz2. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). Decompress again with any standard bunzip2 / bzip2 -d.",
        parameters = schema_json()
    ),
)]
impl Bzip2Compress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("bzip2-compress")?;
    let level = args.level.clamp(1, 9) as usize;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let bz = gizza_ai_bzip2_compress_core::bzip2(&bytes, level)
        .map_err(SkillError::InvalidArgs)?;

    let filename = format!("{base}.bz2");
    let in_len = bytes.len();
    let out_len = bz.len();
    let ratio = if in_len > 0 { 100 - (out_len * 100 / in_len.max(1)) } else { 0 };
    let data_url = format!("data:application/x-bzip2;base64,{}", B64.encode(&bz));
    let for_llm = format!(
        "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes bzip2, ~{ratio}% smaller, block size {level})"
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/x-bzip2".to_string(), filename },
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
                    "level": { "type": "integer", "minimum": 1, "maximum": 9, "default": 9, "description": "Block size 1-9 — each unit is a 100 KB BWT block (1 = least memory, 9 = best ratio; default 9)." }
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
