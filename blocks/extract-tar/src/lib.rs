//! gizza-ai/extract-tar — fetch a tar / tar.gz / tgz archive (URL or attachment
//! ref), list its members, and return the regular files repacked into a ZIP.
//!
//! Pipeline: resolve the source file → `core::extract_to_zip` (pure-Rust
//! tar + flate2 + zip) → base64 envelope (the ZIP as a downloadable file). The
//! `for_llm` text carries the member listing.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP output fits neither the pure-text nor
//! the ffmpeg media page shape — the F3 no-page file-input pattern).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB compressed archive

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. No other parameters: the archive
/// type (tar vs tar.gz/tgz) is auto-detected from its bytes.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractTar;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-tar",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List and extract files from a tar / tar.gz / tgz archive",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "List and extract the files from a tar, tar.gz, or tgz archive, returning them repacked as a ZIP (gzip is auto-detected). The response lists every member (files and directories). Provide the archive as either url (HTTP/HTTPS) or ref (id from a prior tool call). Paths are sanitized (no absolute paths or '..' traversal).",
        parameters = schema_json()
    ),
)]
impl ExtractTar {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("extract-tar")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (zip, ex) =
        gizza_ai_extract_tar_core::extract_to_zip(&bytes).map_err(SkillError::InvalidArgs)?;

    // Build a compact listing for the LLM (cap the number of lines shown).
    let shown = 50usize;
    let mut listing: String = ex
        .entries
        .iter()
        .take(shown)
        .map(|e| {
            if e.is_dir {
                format!("  {}/", e.path.trim_end_matches('/'))
            } else {
                format!("  {} ({} bytes)", e.path, e.size)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if ex.entries.len() > shown {
        listing.push_str(&format!("\n  … and {} more", ex.entries.len() - shown));
    }

    // Output filename: strip a trailing .tar/.tar.gz/.tgz, then add .zip.
    let stem = in_filename
        .strip_suffix(".tar.gz")
        .or_else(|| in_filename.strip_suffix(".tgz"))
        .or_else(|| in_filename.strip_suffix(".tar"))
        .map(|s| format!("{s}.zip"))
        .unwrap_or_else(|| replace_extension(&in_filename, "zip"));

    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let for_llm = format!(
        "extracted {} file(s) from {in_filename} into {stem} ({zip_len}-byte ZIP). Members:\n{listing}",
        ex.file_count
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/zip".to_string(),
            filename: stem,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: descriptor-derived chat schema must match this authored
    /// schema (Input::File url⊕ref oneOf, no other params).
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
