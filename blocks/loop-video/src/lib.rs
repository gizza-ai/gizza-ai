//! gizza-ai/loop-video — fetch a video/GIF URL or attachment ref and repeat it a
//! set number of times or to a target duration via ffmpeg (-stream_loop),
//! returning one continuous file. Source-resolution, ffmpeg dispatch, and
//! envelope-building delegated to block_utils; pure argv builder in core.
//!
//! NOTE: chat ffmpeg is non-functional (Service Worker) — page + CLI surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_loop_video_core::{build_argv, MAX_COUNT, MAX_DURATION};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 50 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    duration: f64,
}
fn default_count() -> u32 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::integer("count")
                .min(1.0)
                .max(MAX_COUNT as f64)
                .describe("How many times to play the clip in total (1-100, default 2). Ignored if duration is set."),
        )
        .param(
            Param::number("duration")
                .min(0.0)
                .max(MAX_DURATION)
                .describe("Target total duration in seconds (loops the clip to fill it). Takes precedence over count when > 0."),
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
        "gif" => "image/gif",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct LoopVideo;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/loop-video",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Loop a video/GIF a number of times or to a duration",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Repeat a video or GIF into one continuous file — either a set number of times (count, 1-100, default 2) or until a target duration in seconds (duration, takes precedence when > 0). Stream-copied (no re-encode), keeps the input format. Provide the video as either url (HTTP/HTTPS) or ref. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl LoopVideo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("loop-video")?;
    if args.duration <= 0.0 && (args.count < 1 || args.count > MAX_COUNT) {
        return Err(SkillError::InvalidArgs(format!(
            "invalid loop-video args: count must be 1-{MAX_COUNT}"
        )));
    }
    if args.duration > MAX_DURATION {
        return Err(SkillError::InvalidArgs(format!(
            "invalid loop-video args: duration must be <= {MAX_DURATION}s"
        )));
    }

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{in_ext}");
    let stream_loop = if args.duration > 0.0 { -1 } else { (args.count.max(1) - 1) as i64 };
    let duration = (args.duration > 0.0).then_some(args.duration);
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, stream_loop, duration);

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_mime = ext_to_video_mime(in_ext);
    let output_size = output.len();
    let suffix = if args.duration > 0.0 {
        format!("-loop-{}s", args.duration)
    } else {
        format!("-loop-{}x", args.count)
    };
    let filename = filename_with_suffix(&in_filename, &suffix, in_ext);
    let for_llm = format!("looped {in_filename} ({output_size} bytes {out_mime})");
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
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "count":    { "type": "integer", "minimum": 1, "maximum": 100, "description": "How many times to play the clip in total (1-100, default 2). Ignored if duration is set." },
                    "duration": { "type": "number", "minimum": 0, "maximum": 3600, "description": "Target total duration in seconds (loops the clip to fill it). Takes precedence over count when > 0." }
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
