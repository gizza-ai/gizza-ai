//! gizza-ai/video-eq-adjust — fetch a video URL or attachment ref, adjust its
//! brightness / contrast / saturation / gamma via ffmpeg's `eq` filter, and
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
use gizza_ai_video_eq_adjust_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn one() -> f64 {
    1.0
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    brightness: f64, // f64 default is 0.0 (identity)
    #[serde(default = "one")]
    contrast: f64,
    #[serde(default = "one")]
    saturation: f64,
    #[serde(default = "one")]
    gamma: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("brightness")
                .default(0.0)
                .min(-1.0)
                .max(1.0)
                .describe("Brightness shift from -1 (darker) to 1 (brighter); 0 keeps the original. Example: 0.1."),
        )
        .param(
            Param::number("contrast")
                .default(1.0)
                .min(0.0)
                .max(4.0)
                .describe("Contrast multiplier from 0 (flat gray) to 4; 1 keeps the original. Example: 1.2."),
        )
        .param(
            Param::number("saturation")
                .default(1.0)
                .min(0.0)
                .max(3.0)
                .describe("Color saturation from 0 (grayscale) to 3 (vivid); 1 keeps the original. Example: 1.4."),
        )
        .param(
            Param::number("gamma")
                .default(1.0)
                .min(0.1)
                .max(10.0)
                .describe("Gamma from 0.1 to 10; 1 keeps the original, below 1 brightens midtones. Example: 0.9."),
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
struct VideoEqAdjust;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-eq-adjust",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Adjust a video's brightness, contrast, saturation, and gamma",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Adjust a video's brightness, contrast, saturation, and gamma in one ffmpeg pass. brightness -1..1 (0=none), contrast 0..4 (1=none), saturation 0..3 (1=none, 0=grayscale), gamma 0.1..10 (1=none). Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoEqAdjust {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-eq-adjust")?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        args.brightness,
        args.contrast,
        args.saturation,
        args.gamma,
    )
    .map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-eq", out_ext);
    let for_llm = format!(
        "adjusted {in_filename} (brightness={} contrast={} saturation={} gamma={}) → {output_size} bytes {out_mime}",
        args.brightness, args.contrast, args.saturation, args.gamma
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "brightness": { "type": "number", "minimum": -1, "maximum": 1, "default": 0.0, "description": "Brightness shift from -1 (darker) to 1 (brighter); 0 keeps the original. Example: 0.1." },
                    "contrast":   { "type": "number", "minimum": 0, "maximum": 4, "default": 1.0, "description": "Contrast multiplier from 0 (flat gray) to 4; 1 keeps the original. Example: 1.2." },
                    "saturation": { "type": "number", "minimum": 0, "maximum": 3, "default": 1.0, "description": "Color saturation from 0 (grayscale) to 3 (vivid); 1 keeps the original. Example: 1.4." },
                    "gamma":      { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "Gamma from 0.1 to 10; 1 keeps the original, below 1 brightens midtones. Example: 0.9." }
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
    fn output_filename_uses_eq_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-eq", "mp4"), "clip-eq.mp4");
    }
}
