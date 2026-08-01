//! gizza-ai/video-chapter-export — read embedded chapter markers from a video
//! and export them as CSV/JSON/CUE/text, without splitting the video.
//!
//! Pipeline: resolve the source video → `core::export_chapters` (pure, hand-
//! written MP4 `chpl` + Matroska EBML chapter parsers) → flat JSON the LLM reads
//! directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (video file input + text output, the no-page
//! file-input pattern, like image-metadata-viewer / detect-file-type). Chapter
//! metadata sits in a small header atom, but the whole file is fetched to reach
//! it, so in-chat use is bounded by the sandbox memory; the CLI has no such cap.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Fetch cap. Chapters live in a tiny header, but the transport hands us the
/// whole file, so this bounds the download. Generous for the CLI; the chat
/// sandbox's own memory limit is the tighter constraint there.
const MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "json".to_string()
}

const FORMAT_DESC: &str = "Output format: json (structured array of chapters with start/end in HH:MM:SS.mmm and milliseconds), csv (spreadsheet columns index,start,end,start_seconds,end_seconds,title), cue (a CUE sheet with one TRACK per chapter), or text (YouTube-style `0:00 Title` timestamp lines). Default json.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::enumv("format", ["json", "csv", "cue", "text"])
            .default("json")
            .describe(FORMAT_DESC),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoChapterExport;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-chapter-export",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export a video's embedded chapter markers to CSV/JSON/CUE/text",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse the chapter markers embedded in a video and export them as CSV, JSON, a CUE sheet, or plain YouTube-style timestamp text — WITHOUT splitting or re-encoding the video. Reads MP4/M4V/M4A/MOV (Nero `chpl` chapter atom) and Matroska/WebM (.mkv/.webm EBML Chapters), returning each chapter's 1-based index, title, and start/end time (normalized to milliseconds and HH:MM:SS.mmm). Returns chapter_count:0 with empty content for a video that has no embedded chapters; errors on a file that is not a recognized MP4 or Matroska container. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl VideoChapterExport {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-chapter-export")?;
    // AssetKind::Any: we sniff the container ourselves (ftyp / EBML) and give a
    // precise error, so we don't reject a video that a static host serves as
    // application/octet-stream.
    let (bytes, _mime, name) = resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let export = gizza_ai_video_chapter_export_core::export_chapters(&bytes, &args.format, &name)
        .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&export)
        .map_err(|e| SkillError::Serialize(format!("serialize video-chapter-export response: {e}")))
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
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["json", "csv", "cue", "text"],
                        "default": "json",
                        "description": "Output format: json (structured array of chapters with start/end in HH:MM:SS.mmm and milliseconds), csv (spreadsheet columns index,start,end,start_seconds,end_seconds,title), cue (a CUE sheet with one TRACK per chapter), or text (YouTube-style `0:00 Title` timestamp lines). Default json."
                    }
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
