//! gizza-ai/video-audio-track-selector — keep exactly one chosen audio track from
//! a video and drop the other audio tracks, via ffmpeg. Lossless (stream-copies
//! the video + the kept audio, no re-encode). Source-resolution, ffmpeg dispatch,
//! and envelope-building delegated to block_utils. NOTE: chat ffmpeg is
//! non-functional (Service Worker) — page + CLI surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_audio_track_selector_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    track: Option<u32>,
    #[serde(default)]
    keep_subtitles: Option<bool>,
    #[serde(default)]
    set_default: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::integer("track")
                .default(0)
                .min(0.0)
                .describe("0-based index of the audio track to KEEP: 0 = first audio track, 1 = second, 2 = third, and so on. Every other audio track is removed. Fails if the video has no audio track at that index (e.g. track=1 on a single-audio file)."),
        )
        .param(
            Param::boolean("keep_subtitles")
                .default(false)
                .describe("Also keep any embedded subtitle tracks (stream-copied). Off by default, so only the video plus the one chosen audio track remain."),
        )
        .param(
            Param::boolean("set_default")
                .default(true)
                .describe("Flag the kept audio track as the default audio disposition so players auto-select it. On by default; turn off to leave disposition flags untouched."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoAudioTrackSelector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-track-selector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Keep one chosen audio track from a video and drop the rest",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Keep exactly one chosen audio track from a video that has multiple audio tracks (e.g. a multi-language file) and drop the other audio tracks. Pick the track by its 0-based index (track=0 keeps the first audio track). Lossless — the video and the kept audio are stream-copied (no re-encode). Optionally keep subtitle tracks too. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioTrackSelector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-track-selector")?;
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let track = args.track.unwrap_or(0);
    let keep_subtitles = args.keep_subtitles.unwrap_or(false);
    let set_default = args.set_default.unwrap_or(true);
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, track, keep_subtitles, set_default).map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-audio-track", out_ext);
    let for_llm = format!("kept audio track {track} of {in_filename} ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "track": { "type": "integer", "default": 0, "minimum": 0, "description": "0-based index of the audio track to KEEP: 0 = first audio track, 1 = second, 2 = third, and so on. Every other audio track is removed. Fails if the video has no audio track at that index (e.g. track=1 on a single-audio file)." },
                    "keep_subtitles": { "type": "boolean", "default": false, "description": "Also keep any embedded subtitle tracks (stream-copied). Off by default, so only the video plus the one chosen audio track remain." },
                    "set_default": { "type": "boolean", "default": true, "description": "Flag the kept audio track as the default audio disposition so players auto-select it. On by default; turn off to leave disposition flags untouched." }
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
