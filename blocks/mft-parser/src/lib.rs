//! gizza-ai/mft-parser — read an NTFS Master File Table (`$MFT`) into structured JSON.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::parse` (the `mft` binary `$MFT` record parser + filtering/aggregation)
//! → flat JSON the LLM reads directly: a summary of counts plus a list of records,
//! each with its MFT entry number, sequence number, in-use/deleted state,
//! file-vs-directory flag, reconstructed full path, logical size, timestomp flag,
//! and both `$STANDARD_INFORMATION` and `$FILE_NAME` timestamp sets.
//!
//! Filters: by path/name substring, files-only or dirs-only, active-only or
//! deleted-only, timestomp suspects only; cap the number of returned records; or
//! switch to `summary=true` for aggregate counts and the file-system time span.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page — a binary file input with structured JSON
//! output fits neither the pure-text page nor the ffmpeg media page shape (the
//! no-page file-input pattern, like `evtx-parser` / `pcap-network-forensics`).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_mft_parser_core::{parse, Include, Options, Status};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB — $MFT files can be large
const DEFAULT_MAX_RECORDS: i64 = 200;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    path_contains: String,
    #[serde(default = "default_include")]
    include: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    only_timestomp: bool,
    #[serde(default = "default_max_records")]
    max_records: i64,
    #[serde(default)]
    summary: bool,
}

fn default_include() -> String {
    "all".to_string()
}
fn default_status() -> String {
    "all".to_string()
}
fn default_max_records() -> i64 {
    DEFAULT_MAX_RECORDS
}

fn parse_include(s: &str) -> Result<Include, SkillError> {
    match s.trim().to_lowercase().as_str() {
        "" | "all" => Ok(Include::All),
        "files" => Ok(Include::Files),
        "dirs" | "directories" => Ok(Include::Dirs),
        other => Err(SkillError::InvalidArgs(format!(
            "invalid include {other:?}: expected all, files, or dirs"
        ))),
    }
}

fn parse_status(s: &str) -> Result<Status, SkillError> {
    match s.trim().to_lowercase().as_str() {
        "" | "all" => Ok(Status::All),
        "active" => Ok(Status::Active),
        "deleted" => Ok(Status::Deleted),
        other => Err(SkillError::InvalidArgs(format!(
            "invalid status {other:?}: expected all, active, or deleted"
        ))),
    }
}

fn build_options(args: &Args) -> Result<Options, SkillError> {
    let path_contains = {
        let p = args.path_contains.trim();
        if p.is_empty() {
            None
        } else {
            Some(p.to_string())
        }
    };
    Ok(Options {
        path_contains,
        include: parse_include(&args.include)?,
        status: parse_status(&args.status)?,
        only_timestomp: args.only_timestomp,
        max_records: args.max_records.max(0) as usize,
        summary: args.summary,
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("path_contains")
                .describe("Only return records whose file name or reconstructed full path contains this substring, case-insensitive, e.g. \"temp\" or \".exe\". Empty (default) returns every record."),
        )
        .param(
            Param::enumv("include", ["all", "files", "dirs"])
                .default("all")
                .describe("Which record kinds to return: \"all\" (default), \"files\" only, or \"dirs\" (directories) only."),
        )
        .param(
            Param::enumv("status", ["all", "active", "deleted"])
                .default("all")
                .describe("Filter by allocation state: \"all\" (default), \"active\" (in-use records only), or \"deleted\" (unallocated records only — recoverable deleted-file entries)."),
        )
        .param(
            Param::boolean("only_timestomp")
                .default(false)
                .describe("Return only timestomp suspects: records whose $STANDARD_INFORMATION creation time is earlier than the $FILE_NAME creation time (a classic anti-forensic backdating tell). Default false."),
        )
        .param(
            Param::integer("max_records")
                .default(DEFAULT_MAX_RECORDS)
                .min(0.0)
                .describe("Maximum number of records to return, in MFT entry order (0 = all). Default 200 keeps output bounded; the response's `matched_records`/`truncated` tell you if more matched."),
        )
        .param(
            Param::boolean("summary")
                .default(false)
                .describe("Return aggregate counts instead of the records: files, directories, active, deleted, timestomp suspects, plus the earliest/latest timestamp over the matched records. Great for triaging a large $MFT before drilling in."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MftParserTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mft-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an NTFS $MFT into records with $STANDARD_INFORMATION and $FILE_NAME timestamps",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse an NTFS Master File Table ($MFT) file into structured JSON for filesystem-timeline / DFIR work. Decodes the 1024-byte MFT records into a flat list — each with its MFT entry (record) number, sequence number, in-use vs deleted (unallocated) state, file-vs-directory flag, reconstructed full path, parent entry number, logical size, a timestomp_suspect flag, and BOTH timestamp sets NTFS keeps per record: $STANDARD_INFORMATION and $FILE_NAME, each with created, modified, mft_modified, and accessed instants (the MACB times). Filter by path/name substring, files-only or dirs-only (include), active-only or deleted-only (status), or only timestomp suspects (SI created before FN created); cap results with max_records; or set summary=true for aggregate counts (files, directories, active, deleted, timestomp suspects) plus the file-system time span instead of the records. Provide the $MFT as url (HTTP/HTTPS) or ref from a prior tool call. Runs locally, pure Rust.",
        parameters = schema_json()
    ),
)]
impl MftParserTool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("mft-parser")?;
    let opts = build_options(&args)?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let report = parse(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&report)
        .map_err(|e| SkillError::Serialize(format!("serialize mft-parser response: {e}")))
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "path_contains": { "type": "string", "description": "Only return records whose file name or reconstructed full path contains this substring, case-insensitive, e.g. \"temp\" or \".exe\". Empty (default) returns every record." },
                    "include": { "type": "string", "enum": ["all", "files", "dirs"], "default": "all", "description": "Which record kinds to return: \"all\" (default), \"files\" only, or \"dirs\" (directories) only." },
                    "status": { "type": "string", "enum": ["all", "active", "deleted"], "default": "all", "description": "Filter by allocation state: \"all\" (default), \"active\" (in-use records only), or \"deleted\" (unallocated records only — recoverable deleted-file entries)." },
                    "only_timestomp": { "type": "boolean", "default": false, "description": "Return only timestomp suspects: records whose $STANDARD_INFORMATION creation time is earlier than the $FILE_NAME creation time (a classic anti-forensic backdating tell). Default false." },
                    "max_records": { "type": "integer", "default": 200, "minimum": 0, "description": "Maximum number of records to return, in MFT entry order (0 = all). Default 200 keeps output bounded; the response's `matched_records`/`truncated` tell you if more matched." },
                    "summary": { "type": "boolean", "default": false, "description": "Return aggregate counts instead of the records: files, directories, active, deleted, timestomp suspects, plus the earliest/latest timestamp over the matched records. Great for triaging a large $MFT before drilling in." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn include_and_status_parse() {
        assert_eq!(parse_include("Files").unwrap(), Include::Files);
        assert_eq!(parse_include("directories").unwrap(), Include::Dirs);
        assert!(parse_include("bogus").is_err());
        assert_eq!(parse_status("Deleted").unwrap(), Status::Deleted);
        assert!(parse_status("bogus").is_err());
    }
}
