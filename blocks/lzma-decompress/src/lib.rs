//! gizza-ai/lzma-decompress — decompress an LZMA `.xz` (or legacy `.lzma`) file
//! back to its original bytes, returned as a downloadable file. The inverse of
//! lzma-compress / the `xz` command-line tool.
//!
//! Pipeline: resolve the source file → `core::lzma_decompress` (pure-Rust
//! `lzma-rust2`, auto-detecting `.xz` vs legacy `.lzma`) → base64 envelope. The
//! output filename is the input name with its `.xz` / `.lzma` suffix stripped.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→file output fits neither the pure-text
//! nor the ffmpeg media page shape — the no-page file-input pattern, like
//! gunzip / lzma-compress).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 128 * 1024 * 1024; // 128 MiB compressed input

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
struct LzmaDecompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lzma-decompress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decompress an LZMA / .xz file",
    requires = ["wafer-run/network"],
    skill(
        description = "Decompress an LZMA file back to its original bytes, returned as a downloadable file. Handles both the modern .xz container (LZMA2, as produced by xz / lzma-compress / 7-Zip) and the legacy raw .lzma stream — the format is auto-detected. The output filename strips the .xz / .lzma suffix. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). To go the other way (compress into .xz) use lzma-compress.",
        parameters = schema_json()
    ),
)]
impl LzmaDecompress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("lzma-decompress")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (out, fmt) =
        gizza_ai_lzma_decompress_core::lzma_decompress(&bytes).map_err(SkillError::InvalidArgs)?;

    // Output filename: strip a recognised compression suffix; else add ".out".
    let filename = strip_suffix(&in_filename);

    let out_len = out.len();
    let data_url = format!("data:application/octet-stream;base64,{}", B64.encode(&out));
    let for_llm = format!(
        "decompressed {} ({} bytes .{}) → {filename} ({out_len} bytes)",
        if in_filename.is_empty() { "file" } else { &in_filename },
        bytes.len(),
        fmt.label()
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/octet-stream".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

/// Strip a `.xz` / `.lzma` / `.txz` suffix to recover the original name.
/// `.txz` (shorthand for `.tar.xz`) becomes `.tar`. Falls back to `<name>.out`.
#[cfg(target_arch = "wasm32")]
fn strip_suffix(name: &str) -> String {
    if name.is_empty() {
        return "output".to_string();
    }
    let lower = name.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix(".txz") {
        return format!("{}.tar", &name[..stem.len()]);
    }
    for suf in [".xz", ".lzma"] {
        if lower.ends_with(suf) {
            return name[..name.len() - suf.len()].to_string();
        }
    }
    format!("{name}.out")
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
