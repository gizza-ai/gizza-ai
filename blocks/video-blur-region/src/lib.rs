//! gizza-ai/video-blur-region — fetch a video URL or attachment ref, blur or
//! pixelate a fixed rectangular region (license plate, name tag, logo) on every
//! frame via ffmpeg, and return an envelope. The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page);
//! source-resolution, ffmpeg dispatch, and envelope-building are delegated to
//! `block_utils`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_blur_region_core::{plan, Mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn default_mode() -> String {
    "blur".into()
}
fn default_strength() -> u32 {
    20
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_strength")]
    strength: u32,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(Param::integer("x").required().min(0.0).describe("Left offset of the region in pixels from the top-left corner. Example: 40."))
        .param(Param::integer("y").required().min(0.0).describe("Top offset of the region in pixels from the top-left corner. Example: 30."))
        .param(Param::integer("width").required().min(1.0).describe("Region width in pixels. Example: 200."))
        .param(Param::integer("height").required().min(1.0).describe("Region height in pixels. Example: 60."))
        .param(
            Param::enumv("mode", ["blur", "pixelate"])
                .default("blur")
                .describe("How to redact the region: \"blur\" (soft Gaussian) or \"pixelate\" (coarse mosaic, harder to reverse). Default: blur."),
        )
        .param(
            Param::integer("strength")
                .default(20)
                .min(1.0)
                .max(100.0)
                .describe("Effect intensity (1-100). For blur it is the Gaussian sigma (higher = softer); for pixelate it is the mosaic block size in pixels (higher = coarser). Default: 20."),
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
struct VideoBlurRegion;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-blur-region",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Blur or pixelate a fixed rectangular region in a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Blur or pixelate a fixed rectangular region (e.g. a license plate, name tag, or logo) on every frame of a video. Give the region as x/y (top-left offset in pixels) plus width/height in pixels; pick mode = blur (soft Gaussian) or pixelate (coarse mosaic) and a strength of 1-100. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoBlurRegion {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-blur-region")?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        args.x,
        args.y,
        args.width,
        args.height,
        mode,
        args.strength,
    )
    .map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let mode_label = match mode {
        Mode::Blur => "blurred",
        Mode::Pixelate => "pixelated",
    };
    let filename = filename_with_suffix(&in_filename, "-blur-region", out_ext);
    let for_llm = format!(
        "{mode_label} the {}x{} region at ({},{}) of {in_filename} ({output_size} bytes {out_mime})",
        args.width, args.height, args.x, args.y
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "x":        { "type": "integer", "minimum": 0, "description": "Left offset of the region in pixels from the top-left corner. Example: 40." },
                    "y":        { "type": "integer", "minimum": 0, "description": "Top offset of the region in pixels from the top-left corner. Example: 30." },
                    "width":    { "type": "integer", "minimum": 1, "description": "Region width in pixels. Example: 200." },
                    "height":   { "type": "integer", "minimum": 1, "description": "Region height in pixels. Example: 60." },
                    "mode":     { "type": "string", "enum": ["blur", "pixelate"], "default": "blur", "description": "How to redact the region: \"blur\" (soft Gaussian) or \"pixelate\" (coarse mosaic, harder to reverse). Default: blur." },
                    "strength": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20, "description": "Effect intensity (1-100). For blur it is the Gaussian sigma (higher = softer); for pixelate it is the mosaic block size in pixels (higher = coarser). Default: 20." }
                },
                "required": ["x", "y", "width", "height"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_blur_region_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-blur-region", "mp4"), "clip-blur-region.mp4");
    }
}
