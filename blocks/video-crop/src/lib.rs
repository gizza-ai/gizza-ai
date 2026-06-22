//! gizza-ai/video-crop — fetch a video URL or attachment ref, crop via ffmpeg,
//! return an envelope. The chat schema is derived from `descriptor()` (single
//! source — shared across chat + CLI + page); source-resolution, ffmpeg
//! dispatch, and envelope-building are delegated to `block_utils`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_crop_core::build_argv;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    width: u32,
    height: u32,
    #[serde(default)]
    x: Option<u32>,
    #[serde(default)]
    y: Option<u32>,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(Param::integer("width").required().min(1.0).describe("Crop width in pixels."))
        .param(Param::integer("height").required().min(1.0).describe("Crop height in pixels."))
        .param(Param::integer("x").min(0.0).describe("Left offset of the crop in pixels (default: centered)."))
        .param(Param::integer("y").min(0.0).describe("Top offset of the crop in pixels (default: centered)."))
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
struct VideoCrop;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-crop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crop a video to a rectangular region",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Crop a video to a width x height rectangle. Optionally give x/y for the top-left offset (in pixels); without them the crop is centered. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoCrop {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-crop")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid video-crop args: width/height must be > 0".into(),
        ));
    }

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{in_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.width, args.height, args.x, args.y);

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, &format!("-crop{}x{}", args.width, args.height), out_ext);
    let for_llm = format!(
        "cropped {in_filename} to {}x{} ({output_size} bytes {out_mime})",
        args.width, args.height
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + width/height/x/y), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Crop width in pixels." },
                    "height": { "type": "integer", "minimum": 1, "description": "Crop height in pixels." },
                    "x":      { "type": "integer", "minimum": 0, "description": "Left offset of the crop in pixels (default: centered)." },
                    "y":      { "type": "integer", "minimum": 0, "description": "Top offset of the crop in pixels (default: centered)." }
                },
                "required": ["width", "height"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_crop_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-crop320x240", "mp4"), "clip-crop320x240.mp4");
    }
}
