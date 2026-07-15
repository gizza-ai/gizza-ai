//! gizza-ai/archive-extractor — fetch an archive (URL or attachment ref),
//! auto-detect its format from the leading bytes, unpack it, and return every
//! extracted file repacked into a single ZIP for download.
//!
//! Handles zip, tar, gzip, bzip2, xz, zstd, and lz4 — including the layered
//! `.tar.gz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` / `.tar.lz4` family. Pure
//! Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP output fits neither the pure-text nor
//! the ffmpeg media page shape — the F3 no-page file-input pattern).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB compressed archive

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. No other parameters: the format
/// is auto-detected from the archive's bytes.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ArchiveExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/archive-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-detect and extract zip, tar, gzip, bzip2, xz, zstd, and lz4 archives",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Auto-detect and extract an archive, returning every file repacked as a single ZIP. The format is detected from the archive's bytes (no filename needed): zip, tar, gzip (.gz), bzip2 (.bz2), xz (.xz), zstd (.zst), and lz4, including the layered .tar.gz / .tar.bz2 / .tar.xz / .tar.zst / .tar.lz4 family. The response lists every member (files and directories). Provide the archive as either url (HTTP/HTTPS) or ref (id from a prior tool call). Paths are sanitized (no absolute paths or '..' traversal).",
        parameters = schema_json()
    ),
)]
impl ArchiveExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Strip the final `.ext` from a filename (for naming a lone decompressed file).
fn strip_final_ext(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _ext)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Derive the output ZIP filename: strip a known archive/compression suffix and
/// add `.zip`; fall back to `archive.zip`.
fn output_zip_name(in_filename: &str) -> String {
    const SUFFIXES: &[&str] = &[
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tar.lz4", ".tgz", ".tbz2", ".tbz",
        ".txz", ".tzst", ".tlz4", ".tar", ".zip", ".gz", ".bz2", ".xz", ".zst", ".lz4",
    ];
    let lower = in_filename.to_lowercase();
    for suf in SUFFIXES {
        if lower.ends_with(suf) {
            let stem = &in_filename[..in_filename.len() - suf.len()];
            if !stem.is_empty() {
                return format!("{stem}.zip");
            }
        }
    }
    let stem = strip_final_ext(in_filename);
    if stem.is_empty() {
        "archive.zip".to_string()
    } else {
        format!("{stem}.zip")
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("archive-extractor")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    // Hint used only when the input is a lone compressor whose payload is not a
    // tar (e.g. `notes.txt.gz` → `notes.txt`); ignored for zip/tar/.tar.* input.
    let name_hint = strip_final_ext(&in_filename).to_string();

    let (zip, ex) = gizza_ai_archive_extractor_core::extract_to_zip(&bytes, &name_hint)
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

    let out_name = output_zip_name(&in_filename);
    let detected = if ex.inner_tar {
        format!("tar (.tar.{})", ex.format.ext())
    } else {
        ex.format.label().to_string()
    };

    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let for_llm = format!(
        "detected {detected}; extracted {} file(s) from {in_filename} into {out_name} ({zip_len}-byte ZIP). Members:\n{listing}",
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

    #[test]
    fn output_zip_name_strips_known_suffixes() {
        assert_eq!(output_zip_name("project.tar.gz"), "project.zip");
        assert_eq!(output_zip_name("logs.tar.zst"), "logs.zip");
        assert_eq!(output_zip_name("bundle.tgz"), "bundle.zip");
        assert_eq!(output_zip_name("data.xz"), "data.zip");
        assert_eq!(output_zip_name("photos.zip"), "photos.zip");
        assert_eq!(output_zip_name("Report.TAR.GZ"), "Report.zip");
        assert_eq!(output_zip_name("noext"), "noext.zip");
        assert_eq!(output_zip_name(""), "archive.zip");
    }

    #[test]
    fn strip_final_ext_works() {
        assert_eq!(strip_final_ext("notes.txt.gz"), "notes.txt");
        assert_eq!(strip_final_ext("data.zst"), "data");
        assert_eq!(strip_final_ext("noext"), "noext");
    }

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
