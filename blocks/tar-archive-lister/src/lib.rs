//! gizza-ai/tar-archive-lister — enumerate the members of a tar (or tar.gz)
//! archive from its headers alone, without unpacking any file content.
//!
//! Thin chat-skill wrapper around `gizza-ai-tar-archive-lister-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! parsing runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    filter: String,
    #[serde(default = "default_true")]
    include_dirs: bool,
    #[serde(default)]
    time_format: String,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_true() -> bool {
    true
}

fn default_limit() -> u64 {
    500
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The tar archive bytes, encoded as a base64 or hex string. Plain .tar and gzip-wrapped .tar.gz/.tgz are both accepted (gzip is auto-detected from the magic bytes). Example: the base64 of a 3-member archive. Maximum 64 MiB after decompression."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("How the archive bytes are encoded in 'input': 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)."),
        )
        .param(
            Param::enumv("output", ["table", "paths", "csv", "json"])
                .default("table")
                .describe("Output shape. 'table' (default) is a `tar -tvf`-style aligned listing (mode string, owner/group, size, mtime, path, link target) with a summary line; 'paths' is one member path per line; 'csv' has the header row path,type,size,mode,uid,gid,uname,gname,mtime,link_target,offset; 'json' is an object with archive totals plus an entries array carrying every header field."),
        )
        .param(
            Param::enumv("sort", ["archive", "path", "size", "mtime", "type"])
                .default("archive")
                .describe("Member ordering. 'archive' (default) keeps the physical order stored in the archive; 'path' sorts alphabetically; 'size' sorts largest first; 'mtime' sorts newest first; 'type' groups by entry kind. Ties break on path."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Optional path filter. A pattern containing '*' or '?' is treated as a glob matched against the whole member path ('*' matches any characters including '/', '?' matches one), e.g. '*.txt' or 'src/*'. A pattern with no wildcard is a plain substring match, e.g. 'README'. Blank (default) lists every member."),
        )
        .param(
            Param::boolean("include_dirs")
                .default(true)
                .describe("When true (default) directory members are listed alongside files. Set false to list only files, links and device nodes."),
        )
        .param(
            Param::enumv("time_format", ["iso", "epoch", "none"])
                .default("iso")
                .describe("How each member's modification time is rendered: 'iso' (default) as 'YYYY-MM-DD HH:MM:SS' in UTC, 'epoch' as Unix seconds, or 'none' to omit the time column entirely."),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(200000.0)
                .default(500)
                .describe("Maximum number of members to return, 1-200000 (default 500). The summary line and the JSON totals always report the true member count, so truncation is visible."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TarArchiveLister;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tar-archive-lister",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List tar/tar.gz members with paths, sizes, modes, owners and types.",
    skill(
        description = "List every member of a tar archive — path, byte size, permission mode, owner/group, entry type, modification time and link target — read straight from the 512-byte tar headers, without unpacking any file content. Pass the archive bytes as base64 (default) or hex in 'input' and set input_format accordingly; plain .tar and gzip-wrapped .tar.gz/.tgz are auto-detected (bzip2/xz/zstd are not decompressed and produce a clear error). output='table' (default) renders a `tar -tvf`-style listing, 'paths' one path per line, 'csv' a spreadsheet-ready table, 'json' a structured object with archive totals and every header field per entry (including mode_string, uid/gid, device major/minor and the header byte offset). sort orders by archive/path/size/mtime/type; filter narrows by glob ('*.txt', 'src/*') or plain substring; include_dirs=false drops directory members; time_format picks iso/epoch/none; limit caps how many members are returned. Understands v7, ustar (incl. the prefix field), GNU (long names via L/K, base-256 numeric fields) and PAX (x/g extended headers) dialects. Returns a clear error for input that is not a tar archive, a truncated or checksum-corrupt header, a ZIP file, or a bzip2/xz/zstd stream.",
        parameters = schema_json()
    ),
)]
impl TarArchiveLister {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tar-archive-lister", |a: Args| {
            gizza_ai_tar_archive_lister_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                &a.sort,
                &a.filter,
                a.include_dirs,
                &a.time_format,
                a.limit as usize,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The tar archive bytes, encoded as a base64 or hex string. Plain .tar and gzip-wrapped .tar.gz/.tgz are both accepted (gzip is auto-detected from the magic bytes). Example: the base64 of a 3-member archive. Maximum 64 MiB after decompression." },
                    "input_format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How the archive bytes are encoded in 'input': 'base64' (default; standard or URL-safe, padding optional) or 'hex' (whitespace, ':' and '-' separators ignored)." },
                    "output": { "type": "string", "enum": ["table", "paths", "csv", "json"], "default": "table", "description": "Output shape. 'table' (default) is a `tar -tvf`-style aligned listing (mode string, owner/group, size, mtime, path, link target) with a summary line; 'paths' is one member path per line; 'csv' has the header row path,type,size,mode,uid,gid,uname,gname,mtime,link_target,offset; 'json' is an object with archive totals plus an entries array carrying every header field." },
                    "sort": { "type": "string", "enum": ["archive", "path", "size", "mtime", "type"], "default": "archive", "description": "Member ordering. 'archive' (default) keeps the physical order stored in the archive; 'path' sorts alphabetically; 'size' sorts largest first; 'mtime' sorts newest first; 'type' groups by entry kind. Ties break on path." },
                    "filter": { "type": "string", "default": "", "description": "Optional path filter. A pattern containing '*' or '?' is treated as a glob matched against the whole member path ('*' matches any characters including '/', '?' matches one), e.g. '*.txt' or 'src/*'. A pattern with no wildcard is a plain substring match, e.g. 'README'. Blank (default) lists every member." },
                    "include_dirs": { "type": "boolean", "default": true, "description": "When true (default) directory members are listed alongside files. Set false to list only files, links and device nodes." },
                    "time_format": { "type": "string", "enum": ["iso", "epoch", "none"], "default": "iso", "description": "How each member's modification time is rendered: 'iso' (default) as 'YYYY-MM-DD HH:MM:SS' in UTC, 'epoch' as Unix seconds, or 'none' to omit the time column entirely." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200000, "default": 500, "description": "Maximum number of members to return, 1-200000 (default 500). The summary line and the JSON totals always report the true member count, so truncation is visible." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
