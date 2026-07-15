//! gizza-ai/tar-archive — bundle several files into a single .tar archive,
//! optionally gzip-compressed to .tar.gz.
//!
//! Loads each source (URL/ref, any bytes) via block-utils, names entries by
//! their resolved filename, and returns a tar (or tar.gz) as a base64 envelope.
//! `Input::None` + a required `files` source_list (like create-zip/merge-pdf).
//! Surfaces: chat + CLI (no page — array input + binary output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

fn default_compression() -> String {
    "none".to_string()
}

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_compression")]
    compression: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 1)
                .required()
                .describe("Ordered list of files to bundle. Each item has exactly one of `url` or `ref`. Entry names come from each file's name; duplicates are made unique."),
        )
        .param(
            Param::enumv("compression", ["none", "gzip", "bzip2", "xz"])
                .default("none")
                .describe("How to compress the whole archive. 'none' (default) is a plain `.tar`; 'gzip' makes `.tar.gz`; 'bzip2' makes `.tar.bz2` (smaller on text, slower); 'xz' makes `.tar.xz` (smallest, slowest)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TarArchive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tar-archive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bundle multiple files into a single .tar (or .tar.gz) archive",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Bundle multiple files into a single downloadable .tar archive, optionally compressed to .tar.gz (gzip), .tar.bz2 (bzip2) or .tar.xz (xz). Provide at least one file; each is a URL or a `ref` to an uploaded attachment. Entry names come from each file's name; duplicate names are made unique. Set `compression` to choose the format (default none = plain .tar).",
        parameters = schema_json()
    ),
)]
impl TarArchive {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("tar-archive")?;
    if args.files.is_empty() {
        return Err(SkillError::InvalidArgs("tar-archive needs at least 1 file".into()));
    }
    use gizza_ai_tar_archive_core::Compression;
    let compression = Compression::parse(&args.compression).map_err(SkillError::InvalidArgs)?;

    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(args.files.len());
    for field in args.files {
        let (bytes, _mime, name) = resolve_source(field.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
        files.push((name, bytes));
    }

    let archive = gizza_ai_tar_archive_core::create_tar(&files, compression)
        .map_err(SkillError::InvalidArgs)?;
    if archive.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output archive is {} bytes, over the {MAX_OUTPUT_BYTES} cap",
            archive.len()
        )));
    }
    let out_len = archive.len();
    let ext = compression.ext();
    let mime = compression.mime();
    let filename = format!("archive.{ext}");
    let encoded = B64.encode(&archive);
    let data_url = format!("data:{mime};base64,{encoded}");

    let env = Envelope {
        for_llm: format!("bundled {} files into a {out_len}-byte {ext} archive ({filename})", files.len()),
        for_ui: ForUi { data_url, mime: mime.to_string(), filename },
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
                    "files": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Ordered list of files to bundle. Each item has exactly one of `url` or `ref`. Entry names come from each file's name; duplicates are made unique.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "compression": { "type": "string", "enum": ["none", "gzip", "bzip2", "xz"], "default": "none", "description": "How to compress the whole archive. 'none' (default) is a plain `.tar`; 'gzip' makes `.tar.gz`; 'bzip2' makes `.tar.bz2` (smaller on text, slower); 'xz' makes `.tar.xz` (smallest, slowest)." }
                },
                "required": ["files"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
