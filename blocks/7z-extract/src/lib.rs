//! gizza-ai/7z-extract — fetch a `.7z` archive (URL or attachment ref),
//! optionally decrypt it with a password, extract every file entirely in
//! memory, and return them repacked into a single ZIP for download.
//!
//! Supports the LZMA and LZMA2 codecs (the 7z defaults) plus AES-256 decryption
//! for password-protected archives. Pure Rust → runs on ALL backends including
//! the chat Service Worker. Surfaces: chat + CLI. No standalone page (a ZIP
//! output fits neither the pure-text nor the ffmpeg media page shape — the same
//! no-page file-input pattern as blocks/archive-extractor).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB compressed archive

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Password for AES-256-encrypted archives; omitted/empty for unencrypted.
    #[serde(default)]
    password: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; a single optional `password`
/// param unlocks AES-256-encrypted archives.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::string("password")
            .describe(
                "Password for an AES-256-encrypted .7z archive. Omit for unencrypted archives.",
            )
            .placeholder("archive password (only for encrypted .7z)"),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SevenZExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/7z-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a .7z archive, including AES-256-encrypted ones, into a downloadable ZIP",
    requires = ["wafer-run/network"],
    skill(
        description = "Extract every file from a .7z (7-Zip) archive and return them repacked as a single ZIP for download. Supports the LZMA and LZMA2 codecs (the 7z defaults) and AES-256-encrypted archives — pass the password via the `password` parameter (for archives created with header encryption / -mhe=on, the password is also required just to list the contents). The response lists every member (files and directories). Provide the archive as either url (HTTP/HTTPS) or ref (id from a prior tool call). Paths are sanitized (no absolute paths or '..' traversal). Not supported: the rarely-used BZip2/Zstd/PPMd/Brotli/Deflate 7z codecs.",
        parameters = schema_json()
    ),
)]
impl SevenZExtract {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("7z-extract")?;
    let password = args.password.unwrap_or_default();
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let (zip, ex) = gizza_ai_7z_extract_core::extract_to_zip(&bytes, &password)
        .map_err(SkillError::InvalidArgs)?;

    // Compact listing for the LLM (cap the number of lines shown).
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

    let out_name = gizza_ai_7z_extract_core::output_zip_name(&in_filename);
    let zip_len = zip.len();
    let encrypted_note = if ex.encrypted {
        " (opened with password)"
    } else {
        ""
    };
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let for_llm = format!(
        "extracted {} file(s) from {in_filename} into {out_name} ({zip_len}-byte ZIP){encrypted_note}. Members:\n{listing}",
        ex.file_count
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/zip".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: descriptor-derived chat schema must match this authored
    /// schema (Input::File url⊕ref oneOf + the optional `password` string).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "password": { "type": "string", "description": "Password for an AES-256-encrypted .7z archive. Omit for unencrypted archives." }
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
