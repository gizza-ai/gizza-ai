//! gizza-ai/video-ken-burns — fetch a still image (URL or attachment ref),
//! animate it with a smooth pan-and-zoom (Ken Burns) move via ffmpeg, and return
//! a short MP4 media envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure `core::plan` argv
//! builder + param validation stay shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_ken_burns_core::{plan, OUT_NAME};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024; // 12 MiB still image
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024; // 32 MiB clip

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    zoom: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    fps: f64,
}

/// Defaults applied when a numeric param arrives as 0 (unset). Kept here so both
/// the run() path and the tests describe the same contract.
const DEF_DURATION: f64 = 5.0;
const DEF_ZOOM: f64 = 1.3;
const DEF_WIDTH: f64 = 1280.0;
const DEF_HEIGHT: f64 = 720.0;
const DEF_FPS: f64 = 30.0;

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv(
                "direction",
                ["zoom-in", "zoom-out", "pan-left", "pan-right", "pan-up", "pan-down"],
            )
            .describe(
                "Ken Burns move (default zoom-in): zoom-in, zoom-out, or pan-left/right/up/down.",
            ),
        )
        .param(
            Param::number("duration")
                .min(1.0)
                .max(30.0)
                .describe("Clip length in seconds, 1-30 (default 5)."),
        )
        .param(
            Param::number("zoom")
                .min(1.0)
                .max(2.0)
                .describe(
                    "Peak zoom factor, 1.0-2.0 (default 1.3). Panning needs > 1 to have room to move.",
                ),
        )
        .param(
            Param::number("width")
                .min(16.0)
                .max(3840.0)
                .describe("Output width in pixels, 16-3840 (default 1280; snapped to an even number)."),
        )
        .param(
            Param::number("height")
                .min(16.0)
                .max(3840.0)
                .describe("Output height in pixels, 16-3840 (default 720; snapped to an even number)."),
        )
        .param(
            Param::number("fps")
                .min(1.0)
                .max(60.0)
                .describe("Frames per second, 1-60 (default 30)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve the effective params (0 = unset → default) shared by run() + tests.
fn resolved(args: &Args) -> (String, f64, f64, u32, u32, f64) {
    let direction = args
        .direction
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "zoom-in".to_string());
    let duration = if args.duration > 0.0 { args.duration } else { DEF_DURATION };
    let zoom = if args.zoom > 0.0 { args.zoom } else { DEF_ZOOM };
    let width = if args.width > 0.0 { args.width } else { DEF_WIDTH };
    let height = if args.height > 0.0 { args.height } else { DEF_HEIGHT };
    let fps = if args.fps > 0.0 { args.fps } else { DEF_FPS };
    (direction, duration, zoom, width.round() as u32, height.round() as u32, fps)
}

#[cfg(target_arch = "wasm32")]
struct VideoKenBurns;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-ken-burns",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Animate a still image with a smooth pan-and-zoom (Ken Burns) effect into a short video clip.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Animate a still image into a short MP4 clip with a smooth pan-and-zoom (Ken Burns) move. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Optional direction (zoom-in [default], zoom-out, pan-left, pan-right, pan-up, pan-down), duration (seconds 1-30, default 5), zoom (peak factor 1.0-2.0, default 1.3), width/height in pixels (default 1280x720), and fps (default 30). Output is an H.264 MP4.",
        parameters = schema_json()
    ),
)]
impl VideoKenBurns {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args + resolve defaults, then validate through the shared core.
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-ken-burns args: {e}")))?;
    let (direction, duration, zoom, width, height, fps) = resolved(&args);

    // 2. Resolve source — URL fetch or attachment lookup (a still image).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always MP4.
    let in_ext = mime_to_ext(&in_mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {in_mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&direction, duration, zoom, width, height, fps, &ffmpeg_in)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-ken-burns args: {e}")))?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope. Output is an MP4 clip.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-kenburns", "mp4");
    let for_llm = format!(
        "made a {duration:.0}s {direction} Ken Burns clip from {in_filename} ({output_size} bytes mp4)"
    );
    let _ = OUT_NAME; // out_name is authoritative from core::plan
    build_media_envelope(&output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "direction": { "type": "string", "enum": ["zoom-in", "zoom-out", "pan-left", "pan-right", "pan-up", "pan-down"], "description": "Ken Burns move (default zoom-in): zoom-in, zoom-out, or pan-left/right/up/down." },
                    "duration":  { "type": "number", "minimum": 1, "maximum": 30, "description": "Clip length in seconds, 1-30 (default 5)." },
                    "zoom":      { "type": "number", "minimum": 1, "maximum": 2, "description": "Peak zoom factor, 1.0-2.0 (default 1.3). Panning needs > 1 to have room to move." },
                    "width":     { "type": "number", "minimum": 16, "maximum": 3840, "description": "Output width in pixels, 16-3840 (default 1280; snapped to an even number)." },
                    "height":    { "type": "number", "minimum": 16, "maximum": 3840, "description": "Output height in pixels, 16-3840 (default 720; snapped to an even number)." },
                    "fps":       { "type": "number", "minimum": 1, "maximum": 60, "description": "Frames per second, 1-60 (default 30)." }
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
    fn resolved_fills_defaults_when_unset() {
        let args: Args = serde_json::from_str(r#"{"url":"http://x/i.png"}"#).unwrap();
        let (dir, dur, zoom, w, h, fps) = resolved(&args);
        assert_eq!(dir, "zoom-in");
        assert_eq!((dur, zoom, w, h, fps), (5.0, 1.3, 1280, 720, 30.0));
    }

    #[test]
    fn resolved_passes_through_explicit_values() {
        let args: Args = serde_json::from_str(
            r#"{"url":"http://x/i.png","direction":"pan-right","duration":8,"zoom":1.5,"width":1920,"height":1080,"fps":24}"#,
        )
        .unwrap();
        let (dir, dur, zoom, w, h, fps) = resolved(&args);
        assert_eq!(dir, "pan-right");
        assert_eq!((dur, zoom, w, h, fps), (8.0, 1.5, 1920, 1080, 24.0));
    }

    #[test]
    fn output_filename_uses_mp4_ext() {
        assert_eq!(filename_with_suffix("beach.jpg", "-kenburns", "mp4"), "beach-kenburns.mp4");
    }
}
