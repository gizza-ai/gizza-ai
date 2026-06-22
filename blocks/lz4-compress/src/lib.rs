//! gizza-ai/lz4-compress — compress a file (or any bytes) into the standard LZ4
//! frame format (.lz4), returned as a downloadable file. LZ4 is optimised for
//! speed: it compresses and decompresses extremely fast at a modest ratio, which
//! makes it ideal for streaming, logs, and real-time pipelines. The inverse is
//! any standard `lz4 -d` / `unlz4`.
//!
//! Pipeline: resolve the source file → `core::lz4` (pure-Rust lz4_flex frame) →
//! base64 envelope. The output is named `<input>.lz4`.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→file output, the no-page file-input
//! pattern, like gzip-compress / bzip2-compress).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. LZ4 has a single fast mode, so
/// there is no level/quality parameter.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Lz4Compress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lz4-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress a file into LZ4 (.lz4)",
    requires = ["wafer-run/network"],
    skill(
        description = "Compress a file (or any bytes) into the standard LZ4 frame format (.lz4), returned for download. LZ4 is optimised for speed — it compresses and decompresses extremely fast at a modest ratio, ideal for logs, streaming, and real-time pipelines. LZ4 has a single fast mode, so there is no level/quality option. The output is named <input>.lz4. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). Decompress again with any standard lz4 -d / unlz4.",
        parameters = schema_json()
    ),
)]
impl Lz4Compress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("lz4-compress")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let base = if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() };
    let compressed = gizza_ai_lz4_compress_core::lz4(&bytes).map_err(SkillError::InvalidArgs)?;

    let filename = format!("{base}.lz4");
    let in_len = bytes.len();
    let out_len = compressed.len();
    // Signed delta so tiny inputs (where the ~15 B LZ4 frame overhead can exceed
    // the savings) report "larger" instead of underflowing an unsigned subtraction.
    let pct = if in_len > 0 {
        100 - (out_len as i64 * 100 / in_len as i64)
    } else {
        0
    };
    let size_note = if pct >= 0 {
        format!("~{pct}% smaller")
    } else {
        format!("~{}% larger (frame overhead on tiny input)", -pct)
    };
    let data_url = format!("data:application/x-lz4;base64,{}", B64.encode(&compressed));
    let for_llm = format!(
        "compressed {base} ({in_len} bytes) → {filename} ({out_len} bytes LZ4, {size_note})"
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/x-lz4".to_string(), filename },
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
