//! gizza-ai/video-freeze-frame — hold a chosen frame as a still for a set number
//! of seconds inside a video (the freeze-frame effect), via ffmpeg.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Timing validation and the
//! pure argv/filtergraph builder live in `core`, shared with the page.
//!
//! The video plays to `time`, freezes that frame for `duration` seconds, then
//! resumes — the output is `duration` seconds longer than the source and is
//! re-encoded to H.264. AUDIO IS DROPPED (a video-only visual effect). NOTE: chat
//! ffmpeg is non-functional (the chat runtime is a Service Worker where ffmpeg
//! can't load), so the supported surfaces are the standalone page and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_freeze_frame_core::{plan, MAX_DURATION, MAX_TIME};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 50 * 1024 * 1024;

fn default_time() -> f64 {
    1.0
}
fn default_duration() -> f64 {
    2.0
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_time")]
    time: f64,
    #[serde(default = "default_duration")]
    duration: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("time")
                .min(0.0)
                .max(MAX_TIME)
                .default(1.0)
                .describe("Timestamp of the frame to freeze, in seconds from the start (e.g. 2.5). Default 1. The video plays normally up to here, then this frame is held still. If it is past the end of the clip, the last frame is held."),
        )
        .param(
            Param::number("duration")
                .min(0.0)
                .max(MAX_DURATION)
                .default(2.0)
                .describe("How long to hold the frozen frame, in seconds (0-60). Default 2. The output ends up this many seconds longer than the source."),
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
struct VideoFreezeFrame;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-freeze-frame",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hold a chosen frame as a still for N seconds inside a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Create a freeze-frame effect: the video plays normally up to a chosen timestamp (time, seconds), holds that frame as a still for a set number of seconds (duration, 0-60), then resumes from the same point. The output is `duration` seconds longer than the source. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). The frame is held at the source frame rate (no fps guessing) and the video re-encodes to H.264; the input container (mp4/mov/mkv) is kept when it can hold H.264, otherwise the output is mp4. Audio is dropped — the freeze is a video-only visual effect. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoFreezeFrame {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-freeze-frame")?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.time, args.duration).map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-freeze", out_ext);
    let for_llm = format!(
        "froze the frame at {}s for {}s in {in_filename} ({output_size} bytes {out_mime})",
        args.time, args.duration
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + time/duration), so any future change
    /// to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "time":     { "type": "number", "minimum": 0, "maximum": 36000, "default": 1.0, "description": "Timestamp of the frame to freeze, in seconds from the start (e.g. 2.5). Default 1. The video plays normally up to here, then this frame is held still. If it is past the end of the clip, the last frame is held." },
                    "duration": { "type": "number", "minimum": 0, "maximum": 60, "default": 2.0, "description": "How long to hold the frozen frame, in seconds (0-60). Default 2. The output ends up this many seconds longer than the source." }
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
    fn output_filename_uses_freeze_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-freeze", "mp4"), "clip-freeze.mp4");
    }
}
