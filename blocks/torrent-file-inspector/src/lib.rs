//! gizza-ai/torrent-file-inspector — parse a `.torrent` (bencode) file and
//! report its metainfo: suggested name, the file list with per-file sizes, the
//! tracker announce URLs, piece length + piece count, total size, the BitTorrent
//! v1 info-hash (SHA-1 of the bencoded `info` dict), and the optional comment /
//! created-by / creation-date fields.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::inspect` (pure, hand-rolled bencode + sha1) → flat JSON the LLM reads
//! directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→report fits neither the pure-text page
//! nor the ffmpeg file→media page shape — the F3 "no-page file-input" pattern,
//! like detect-file-type / web-fetch).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // .torrent files are small, but allow large piece tables

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct FileEntry {
    /// Path of the file within the torrent (root dir prefixed for multi-file torrents).
    path: String,
    /// File size in bytes.
    bytes: i64,
}

#[derive(Serialize)]
struct Resp {
    /// Suggested display name (the file name, or the root directory for multi-file torrents).
    name: String,
    /// BitTorrent v1 info-hash: lowercase hex SHA-1 of the bencoded `info` dictionary.
    info_hash: String,
    /// True when the torrent contains more than one file.
    is_multi_file: bool,
    /// Number of files in the torrent.
    file_count: usize,
    /// The files with their individual sizes.
    files: Vec<FileEntry>,
    /// Total size across all files, in bytes.
    total_bytes: i64,
    /// Bytes per piece (`piece length`).
    piece_length: i64,
    /// Number of pieces (length of `pieces` / 20).
    piece_count: usize,
    /// Whether the torrent is flagged private (no DHT/PEX).
    private: bool,
    /// Tracker announce URLs, deduplicated, in declaration order.
    trackers: Vec<String>,
    /// Optional free-text comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
    /// Optional creating program/agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    created_by: Option<String>,
    /// Optional creation time as a Unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    creation_date: Option<i64>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a `.torrent` arrives via URL
/// fetch or an attachment ref. No other parameters: parsing is fully automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TorrentFileInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/torrent-file-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a .torrent file and show its name, files, trackers, and info-hash",
    requires = ["wafer-run/network"],
    skill(
        description = "Inspect a BitTorrent .torrent file: parse its bencoded metainfo and report the suggested name, the file list with each file's size, the total size, the tracker announce URLs (from announce and announce-list), the piece length and piece count, whether it is a private torrent, the BitTorrent v1 info-hash (SHA-1 of the bencoded info dictionary, the magnet-link btih), and the optional comment, created-by, and creation-date fields. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl TorrentFileInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("torrent-file-inspector")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let info = gizza_ai_torrent_file_inspector_core::inspect(&bytes)
        .map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        name: info.name,
        info_hash: info.info_hash,
        is_multi_file: info.is_multi_file,
        file_count: info.files.len(),
        files: info
            .files
            .into_iter()
            .map(|f| FileEntry { path: f.path, bytes: f.length })
            .collect(),
        total_bytes: info.total_length,
        piece_length: info.piece_length,
        piece_count: info.piece_count,
        private: info.private,
        trackers: info.trackers,
        comment: info.comment,
        created_by: info.created_by,
        creation_date: info.creation_date,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize torrent-file-inspector response: {e}")))
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
