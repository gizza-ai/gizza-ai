//! gizza-ai/file-hash-dedupe — content-address a set of files by hash and
//! report byte-identical duplicates regardless of filename, with a suggested
//! keep/delete list and the bytes each deletion would reclaim.
//!
//! Pipeline: resolve each file source (URL/ref, ANY bytes) one at a time →
//! `core::digest_file` (chosen digest + an internal SHA-256 confirmation, then
//! the bytes are dropped) → pure `core::dedupe` (group by size+SHA-256, apply
//! the keep policy, roll up reclaimable bytes) → JSON report. `Input::None` +
//! a required `files` source_list (like duplicate-image-finder / image-collage).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page: the input is an ARRAY of sources, which the
//! page generator's scalar `[[input]]` controls cannot express (same conclusion
//! as duplicate-image-finder), and the output is a JSON report.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, respond_ok, AssetKind, Source};
use gizza_ai_block_utils::{Input, Param, SkillError, SourceFields, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_file_hash_dedupe_core::{dedupe, digest_file, Algorithm, FileEntry, Keep, MAX_FILES};
use serde::Deserialize;
use wafer_sdk::*;

/// Each file is capped at 32 MiB on the wire. Only ONE file's bytes are held at
/// a time (hash, then drop), so peak memory is one file, not the whole set.
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default)]
    include_unique: bool,
}
fn default_algorithm() -> String {
    "sha256".to_string()
}
fn default_keep() -> String {
    "first".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Two or more file sources (any type: documents, archives, images, media, binaries) to content-address and compare. Each item has exactly one of `url` or `ref`. Up to 50 files, 32 MiB each."),
        )
        .param(
            Param::enumv("algorithm", ["sha256", "sha1", "md5", "sha512", "blake3", "crc32"])
                .default("sha256")
                .describe("Digest reported for each file: sha256 (default, the safe general choice), sha1, md5, sha512, blake3 (fastest modern hash) or crc32 (short 8-hex checksum, matches zip/gzip). Duplicate detection itself always confirms byte identity with size + SHA-256, so a weak choice like md5 or crc32 changes only the reported hash, never the grouping."),
        )
        .param(
            Param::enumv("keep", ["first", "last", "shortest-name"])
                .default("first")
                .describe("Which copy in each duplicate group to suggest keeping: first (default, the earliest one listed), last (the latest one listed), or shortest-name (the shortest source label — prefers `photo.jpg` over `photo (copy) (1).jpg`). Every other member is listed under `delete`."),
        )
        .param(
            Param::boolean("include_unique")
                .default(false)
                .describe("false (default) lists only the files that have at least one duplicate, like a duplicates-only report. true lists every input file, with `group` omitted for files that are unique. Group and summary counts are identical either way."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileHashDedupe;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-hash-dedupe",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find byte-identical duplicate files in a set by content hash",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Content-address a set of files by hash and report the byte-identical duplicates, regardless of filename. Provide `files` as a list of 2+ sources (each a url or a `ref`; any file type, up to 50 files of 32 MiB each). `algorithm` picks the digest reported per file — sha256 (default), sha1, md5, sha512, blake3 or crc32. `keep` picks which copy in each duplicate group to suggest keeping: first (default), last, or shortest-name. `include_unique` (default false) lists only duplicated files; set true to list every file. The result gives each file (index, source, size, hash, group), the duplicate groups (shared hash and size, member indices, suggested keep, delete list, reclaimable bytes), and a summary with distinct/unique/duplicate counts, total and reclaimable bytes and the wasted percent. Matching is EXACT: two files group only when their size and SHA-256 both agree, so a weak `algorithm` choice never causes a false duplicate — a chosen-digest match that fails that confirmation is reported as `summary.hash_collisions` instead. Similar-but-not-identical files (a re-encoded or resized image) are NOT matched; use duplicate-image-finder for perceptual near-duplicates. The tool only reports — it never deletes or moves anything.",
        parameters = schema_json()
    ),
)]
impl FileHashDedupe {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;

    let args: Args = serde_json::from_slice(&body).invalid_args("file-hash-dedupe")?;
    if args.files.len() < 2 {
        return Err(SkillError::InvalidArgs(
            "file-hash-dedupe needs at least 2 files to compare".into(),
        ));
    }
    // Cap BEFORE fetching, so an oversized request never triggers 50+ fetches.
    if args.files.len() > MAX_FILES {
        return Err(SkillError::InvalidArgs(format!(
            "too many files: {} exceeds the {MAX_FILES}-file cap",
            args.files.len()
        )));
    }
    let algorithm = Algorithm::parse(&args.algorithm).map_err(SkillError::InvalidArgs)?;
    let keep = Keep::parse(&args.keep).map_err(SkillError::InvalidArgs)?;

    let mut entries: Vec<FileEntry> = Vec::with_capacity(args.files.len());
    for (i, field) in args.files.into_iter().enumerate() {
        let source = field.into_inner();
        let from_source = source_label(&source);
        let (bytes, _mime, name) = resolve_source(source, AssetKind::Any, MAX_INPUT_BYTES)?;
        // Prefer the resolved filename when it carries an extension (a real
        // name); otherwise fall back to a label derived from the URL/ref, then
        // the index. This keeps the keep/delete list identifiable.
        let label = if name.contains('.') && !name.trim().is_empty() {
            name
        } else if !from_source.is_empty() {
            from_source
        } else {
            format!("file {i}")
        };
        let size = bytes.len();
        let (hash, confirm) = digest_file(&bytes, algorithm);
        // Drop this file's bytes before fetching the next one — peak memory
        // stays at a single file regardless of how many are compared.
        drop(bytes);
        entries.push(FileEntry {
            label,
            bytes: size,
            hash,
            confirm,
        });
    }

    let report = dedupe(&entries, algorithm, keep, args.include_unique)
        .map_err(SkillError::InvalidArgs)?;
    respond_ok(&report)
}

/// A human-facing label for a source, used to identify which file to delete:
/// the ref id, a URL's filename segment, or the host+path (scheme stripped).
#[cfg(target_arch = "wasm32")]
fn source_label(source: &Source) -> String {
    match source {
        Source::Ref(id) => id.clone(),
        Source::Url(u) => {
            let no_query = u.split(['?', '#']).next().unwrap_or(u);
            let seg = no_query.rsplit('/').next().unwrap_or("");
            if seg.contains('.') {
                seg.to_string()
            } else {
                let short = u
                    .strip_prefix("https://")
                    .or_else(|| u.strip_prefix("http://"))
                    .unwrap_or(u);
                short.chars().take(80).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Two or more file sources (any type: documents, archives, images, media, binaries) to content-address and compare. Each item has exactly one of `url` or `ref`. Up to 50 files, 32 MiB each.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "algorithm": {
                        "type": "string",
                        "enum": ["sha256", "sha1", "md5", "sha512", "blake3", "crc32"],
                        "default": "sha256",
                        "description": "Digest reported for each file: sha256 (default, the safe general choice), sha1, md5, sha512, blake3 (fastest modern hash) or crc32 (short 8-hex checksum, matches zip/gzip). Duplicate detection itself always confirms byte identity with size + SHA-256, so a weak choice like md5 or crc32 changes only the reported hash, never the grouping."
                    },
                    "keep": {
                        "type": "string",
                        "enum": ["first", "last", "shortest-name"],
                        "default": "first",
                        "description": "Which copy in each duplicate group to suggest keeping: first (default, the earliest one listed), last (the latest one listed), or shortest-name (the shortest source label — prefers `photo.jpg` over `photo (copy) (1).jpg`). Every other member is listed under `delete`."
                    },
                    "include_unique": {
                        "type": "boolean",
                        "default": false,
                        "description": "false (default) lists only the files that have at least one duplicate, like a duplicates-only report. true lists every input file, with `group` omitted for files that are unique. Group and summary counts are identical either way."
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
