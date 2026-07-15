//! gizza-ai/gunzip — decompress a gzip (.gz) file or blob back to its original
//! bytes, returned as a downloadable file.
//!
//! Pipeline: resolve the source file → `core::gunzip` (pure-Rust flate2) → base64
//! envelope. The output filename is the gzip header's embedded name if present,
//! else the input name with `.gz` stripped.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→file output fits neither the pure-text
//! nor the ffmpeg media page shape — the F3 no-page file-input pattern).
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

/// `Input::File` emits the `url`⊕`ref` `oneOf`. No other parameters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Gunzip;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gunzip",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decompress a gzip (.gz) file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Decompress a gzip (.gz) file or blob back to its original bytes, returned as a downloadable file. The original filename is recovered from the gzip header when present (otherwise the .gz suffix is stripped). Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). For .tar.gz archives use extract-tar instead.",
        parameters = schema_json()
    ),
)]
impl Gunzip {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("gunzip")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (out, header_name) =
        gizza_ai_gunzip_core::gunzip(&bytes).map_err(SkillError::InvalidArgs)?;

    // Output filename: prefer the gzip header's embedded name, else strip ".gz".
    let filename = header_name.unwrap_or_else(|| {
        in_filename
            .strip_suffix(".gz")
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if in_filename.is_empty() {
                    "output".to_string()
                } else {
                    format!("{in_filename}.out")
                }
            })
    });

    let out_len = out.len();
    let data_url = format!("data:application/octet-stream;base64,{}", B64.encode(&out));
    let for_llm = format!(
        "decompressed {in_filename} → {filename} ({out_len} bytes, from {} compressed)",
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
