//! gizza-ai/video-to-gif — fetch a video URL or attachment ref, convert a
//! section of it into an optimized animated GIF, return a media envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure `core` argv
//! builder + param validation stay shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_to_gif_core::{build_argv, OUT_NAME};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    fps: f64,
    #[serde(default)]
    width: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("start")
                .min(0.0)
                .describe("Start time in seconds (default 0, from the beginning)."),
        )
        .param(
            Param::number("duration")
                .min(0.0)
                .describe("Clip length in seconds (default 0 = to the end of the video)."),
        )
        .param(
            Param::number("fps")
                .min(0.0)
                .max(60.0)
                .describe("Frames per second of the GIF (default 12). Lower fps = smaller file."),
        )
        .param(
            Param::number("width")
                .min(0.0)
                .max(4096.0)
                .describe("Output width in pixels; height scales to keep aspect ratio (default 0 = keep source size)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToGif;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-gif",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a section of a video into an optimized animated GIF",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert a section of a video into an optimized animated GIF. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Optional start (seconds, default 0), duration (seconds, default 0 = to the end), fps (default 12), and width in pixels (default 0 = keep source size; height scales to preserve aspect ratio). Output is a looping GIF, made with a per-clip palette (palettegen/paletteuse) for high quality at a small size.",
        parameters = schema_json()
    ),
)]
impl VideoToGif {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse + validate args (shared core validator).
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-gif args: {e}")))?;
    gizza_ai_video_to_gif_core::plan("in.mp4", args.start, args.duration, args.fps, args.width)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-gif args: {e}")))?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always GIF.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = OUT_NAME.to_string();
    let width = args.width.round() as u32;
    let argv = build_argv(&ffmpeg_in, args.start, args.duration, args.fps, width);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope. Output is GIF.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "", "gif");
    let for_llm = format!(
        "made an animated GIF from {} ({} bytes gif)",
        in_filename, output_size
    );
    build_media_envelope(&output, "image/gif", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM-facing contract is stable.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "start":    { "type": "number", "minimum": 0, "description": "Start time in seconds (default 0, from the beginning)." },
                    "duration": { "type": "number", "minimum": 0, "description": "Clip length in seconds (default 0 = to the end of the video)." },
                    "fps":      { "type": "number", "minimum": 0, "maximum": 60, "description": "Frames per second of the GIF (default 12). Lower fps = smaller file." },
                    "width":    { "type": "number", "minimum": 0, "maximum": 4096, "description": "Output width in pixels; height scales to keep aspect ratio (default 0 = keep source size)." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_gif_ext() {
        assert_eq!(filename_with_suffix("clip.webm", "", "gif"), "clip.gif");
    }
}
