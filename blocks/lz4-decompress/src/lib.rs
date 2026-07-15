//! gizza-ai/lz4-decompress — decompress a standard LZ4 frame file (.lz4) back to
//! its original bytes, returned as a downloadable file. LZ4 is optimised for
//! speed: it compresses and decompresses extremely fast at a modest ratio, which
//! makes it ideal for streaming, logs, and real-time pipelines. This is the exact
//! inverse of the lz4-compress tool (and of any standard `lz4 -d` / `unlz4`).
//!
//! Pipeline: resolve the source file → `core::unlz4` (pure-Rust lz4_flex frame
//! decoder) → base64 envelope. The output filename has the `.lz4` suffix stripped.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gunzip / lz4-compress).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB compressed input

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. No other parameters — LZ4 frame
/// decoding is fully determined by the input.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Lz4Decompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lz4-decompress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decompress an LZ4 (.lz4) file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Decompress a standard LZ4 frame file (.lz4) back to its original bytes, returned as a downloadable file. This is the exact inverse of the lz4-compress tool and of any standard lz4 -d / unlz4. LZ4 is optimised for speed — fast to compress and decompress at a modest ratio, ideal for logs, streaming, and real-time pipelines. The .lz4 suffix is stripped from the output filename. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). For .tar.lz4 archives, decompress first then use extract-tar.",
        parameters = schema_json()
    ),
)]
impl Lz4Decompress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("lz4-decompress")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let out = gizza_ai_lz4_decompress_core::unlz4(&bytes).map_err(SkillError::InvalidArgs)?;

    // Output filename: strip a trailing ".lz4" if present, else fall back.
    let filename = in_filename
        .strip_suffix(".lz4")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if in_filename.is_empty() {
                "output".to_string()
            } else {
                format!("{in_filename}.out")
            }
        });

    let out_len = out.len();
    let data_url = format!("data:application/octet-stream;base64,{}", B64.encode(&out));
    let for_llm = format!(
        "decompressed {in_filename} ({} bytes LZ4) → {filename} ({out_len} bytes)",
        bytes.len()
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
