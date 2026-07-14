//! gizza-ai/dir-checksum-report — turn a batch of uploaded files into one
//! checksum manifest (Markdown table or CSV) of filename, size, and the
//! requested digests (CRC-32/MD5/SHA-1/SHA-256/SHA-512), with a "Duplicate
//! files" section (Markdown only) for any files whose digests all match.
//! Loads each source (URL/ref) via block-utils, computes digests + renders
//! with the pure core, returns the report as text `{result}`. `Input::None` +
//! a required `files` source_list (like csv-merge/merge-pdf). Surfaces: chat
//! + CLI (no page — a multi-file report needs more than one upload slot, and
//! the page driver only supports a single file input).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{respond_ok, Input, Param, SkillError, SourceFields, ToolDescriptor};
use gizza_ai_dir_checksum_report_core::{build_report, parse_algorithms, Format, SortBy};
use serde::Deserialize;
use wafer_sdk::*;

/// Per-file byte cap (16 MiB) — generous for real files while bounding the
/// memory a single report can pull into the wasm sandbox.
const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

fn default_algorithms() -> String {
    "crc32,sha256".to_string()
}

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_algorithms")]
    algorithms: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    sort_by: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Two or more files to checksum. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::string("algorithms")
                .default("crc32,sha256")
                .describe(
                    "Comma-separated digest algorithms to include as report columns, in order: any of crc32, md5, sha1, sha256, sha512. Default 'crc32,sha256'.",
                ),
        )
        .param(
            Param::enumv("format", ["markdown", "csv"])
                .default("markdown")
                .describe(
                    "Report shape: 'markdown' (a table plus a 'Duplicate files' section listing any files whose digests all match) or 'csv' (header + one row per file, no duplicate section). Default 'markdown'.",
                ),
        )
        .param(
            Param::enumv("sort_by", ["name", "size"])
                .default("name")
                .describe("Row order: 'name' (case-insensitive alphabetical) or 'size' (ascending bytes). Default 'name'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DirChecksumReport;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dir-checksum-report",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Checksum report for a batch of files",
    requires = ["wafer-run/network"],
    skill(
        description = "Produce a checksum manifest for a batch of files: a Markdown table (or CSV) of filename, size, and the requested digests (any of crc32, md5, sha1, sha256, sha512; default crc32+sha256), plus a 'Duplicate files' section (Markdown only) grouping files whose digests all match. Provide at least two file sources; each is a URL or a `ref` to an uploaded attachment.",
        parameters = schema_json()
    ),
)]
impl DirChecksumReport {
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

    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid dir-checksum-report args: {e}")))?;
    if args.files.len() < 2 {
        return Err(SkillError::InvalidArgs(format!(
            "dir-checksum-report needs at least 2 files, got {}",
            args.files.len()
        )));
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(args.files.len());
    for field in args.files {
        let (bytes, _mime, name) = resolve_source(field.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
        files.push((name, bytes));
    }

    let algorithms = parse_algorithms(&args.algorithms).map_err(SkillError::InvalidArgs)?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let sort_by = SortBy::parse(&args.sort_by).map_err(SkillError::InvalidArgs)?;

    let report = build_report(&files, &algorithms, format, sort_by).map_err(SkillError::InvalidArgs)?;
    respond_ok(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chat-facing schema is the single source of truth for the chat LLM
    /// tool call AND the CLI — this test guards against drift between
    /// `descriptor()` and the schema an integration expects.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Two or more files to checksum. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "algorithms": { "type": "string", "default": "crc32,sha256", "description": "Comma-separated digest algorithms to include as report columns, in order: any of crc32, md5, sha1, sha256, sha512. Default 'crc32,sha256'." },
                    "format": { "type": "string", "enum": ["markdown", "csv"], "default": "markdown", "description": "Report shape: 'markdown' (a table plus a 'Duplicate files' section listing any files whose digests all match) or 'csv' (header + one row per file, no duplicate section). Default 'markdown'." },
                    "sort_by": { "type": "string", "enum": ["name", "size"], "default": "name", "description": "Row order: 'name' (case-insensitive alphabetical) or 'size' (ascending bytes). Default 'name'." }
                },
                "required": ["files"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_two_url_files_with_defaults() {
        let a: Args = serde_json::from_str(
            r#"{"files":[{"url":"https://x/a.txt"},{"url":"https://x/b.txt"}]}"#,
        )
        .unwrap();
        assert_eq!(a.files.len(), 2);
        assert_eq!(a.algorithms, "crc32,sha256");
        assert_eq!(a.format, "");
        assert_eq!(a.sort_by, "");
    }

    #[test]
    fn args_parse_mixed_url_and_ref_with_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"files":[{"url":"https://x/a.txt"},{"ref":"call_7"}],"algorithms":"sha256","format":"csv","sort_by":"size"}"#,
        )
        .unwrap();
        assert_eq!(a.files.len(), 2);
        assert_eq!(a.algorithms, "sha256");
        assert_eq!(a.format, "csv");
        assert_eq!(a.sort_by, "size");
    }

    #[test]
    fn args_reject_item_with_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"files":[{"url":"u","ref":"r"}]}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
