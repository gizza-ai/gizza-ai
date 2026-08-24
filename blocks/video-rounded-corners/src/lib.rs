//! gizza-ai/video-rounded-corners — fetch a video URL or attachment ref, round
//! the corners of every frame with an ffmpeg `geq` alpha mask, and return an
//! envelope. The chat schema is derived from `descriptor()` (single source —
//! shared across chat + CLI + page); source-resolution, ffmpeg dispatch and
//! envelope-building are delegated to `block_utils`.
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
use gizza_ai_video_rounded_corners_core::{plan, DEFAULT_QUALITY, DEFAULT_RADIUS};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn default_radius() -> f64 {
    DEFAULT_RADIUS
}
fn default_unit() -> String {
    "px".into()
}
fn default_corners() -> String {
    "all".into()
}
fn default_background() -> String {
    "transparent".into()
}
fn default_format() -> String {
    "webm".into()
}
fn default_quality() -> u32 {
    DEFAULT_QUALITY
}
fn default_keep_audio() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_radius")]
    radius: f64,
    #[serde(default = "default_unit")]
    radius_unit: String,
    #[serde(default = "default_corners")]
    corners: String,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_quality")]
    quality: u32,
    #[serde(default = "default_keep_audio")]
    keep_audio: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("radius")
                .default(24.0)
                .min(1.0)
                .max(1000.0)
                .describe("Corner radius. In pixels by default (e.g. 40 for a typical card); with radius_unit=percent it is a share of the SHORTER side, where 50 fully rounds the ends. Clamped to half the shorter side. Default: 24."),
        )
        .param(
            Param::enumv("radius_unit", ["px", "percent"])
                .default("px")
                .describe("How radius is measured: \"px\" (absolute pixels, 1-1000) or \"percent\" (1-50 percent of the shorter side, resolution-independent). Default: px."),
        )
        .param(
            Param::enumv("corners", ["all", "top", "bottom", "left", "right"])
                .default("all")
                .describe("Which corners to round: \"all\" (four), \"top\" (both top corners), \"bottom\", \"left\" or \"right\". Default: all."),
        )
        .param(
            Param::string("background")
                .default("transparent")
                .describe("What fills the cut-off corners: \"transparent\" (default, needs format webm or mov) or a solid color as a CSS name (black, white, navy) or hex (#1E90FF). A solid color lets you export mp4."),
        )
        .param(
            Param::enumv("format", ["webm", "mp4", "mov"])
                .default("webm")
                .describe("Output container/codec: \"webm\" (VP9 — web-playable AND keeps transparency), \"mp4\" (H.264 — universal but has no alpha, so it needs a background color), or \"mov\" (ProRes 4444 with alpha for editors; very large files). Default: webm."),
        )
        .param(
            Param::integer("quality")
                .default(75)
                .min(1.0)
                .max(100.0)
                .describe("Encode quality 1-100 (higher = better and larger). Maps to the codec's CRF; ignored for mov/ProRes, which is always near-lossless. Default: 75."),
        )
        .param(
            Param::boolean("keep_audio")
                .default(true)
                .describe("Keep the original audio track (true, default) or drop it (false). Silent clips are unaffected."),
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
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoRoundedCorners;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-rounded-corners",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Round the corners of a video into a card-style clip",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Round the corners of every frame of a video so the clip looks like a card, with transparent or solid-colored corners. Set radius (pixels, or percent of the shorter side), pick which corners to round (all/top/bottom/left/right), and choose background = transparent or a color. Transparency needs format webm (VP9) or mov (ProRes 4444); mp4 requires a background color. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoRoundedCorners {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-rounded-corners")?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        args.radius,
        &args.radius_unit,
        &args.corners,
        &args.background,
        &args.format,
        args.quality,
        args.keep_audio,
    )
    .map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out
        .rsplit_once('.')
        .map(|(_, e)| e)
        .unwrap_or("webm");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let corner_fill = if args.background.trim().is_empty()
        || args.background.eq_ignore_ascii_case("transparent")
        || args.background.eq_ignore_ascii_case("none")
    {
        "transparent".to_string()
    } else {
        args.background.trim().to_string()
    };
    let unit_label = if args.radius_unit.trim().eq_ignore_ascii_case("percent") {
        "%"
    } else {
        "px"
    };
    let filename = filename_with_suffix(&in_filename, "-rounded", out_ext);
    let for_llm = format!(
        "rounded the {} corners of {in_filename} at radius {}{unit_label} with {corner_fill} corners ({output_size} bytes {out_mime})",
        args.corners.trim(),
        args.radius
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
                    "url":         { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "radius":      { "type": "number", "minimum": 1, "maximum": 1000, "default": 24.0, "description": "Corner radius. In pixels by default (e.g. 40 for a typical card); with radius_unit=percent it is a share of the SHORTER side, where 50 fully rounds the ends. Clamped to half the shorter side. Default: 24." },
                    "radius_unit": { "type": "string", "enum": ["px", "percent"], "default": "px", "description": "How radius is measured: \"px\" (absolute pixels, 1-1000) or \"percent\" (1-50 percent of the shorter side, resolution-independent). Default: px." },
                    "corners":     { "type": "string", "enum": ["all", "top", "bottom", "left", "right"], "default": "all", "description": "Which corners to round: \"all\" (four), \"top\" (both top corners), \"bottom\", \"left\" or \"right\". Default: all." },
                    "background":  { "type": "string", "default": "transparent", "description": "What fills the cut-off corners: \"transparent\" (default, needs format webm or mov) or a solid color as a CSS name (black, white, navy) or hex (#1E90FF). A solid color lets you export mp4." },
                    "format":      { "type": "string", "enum": ["webm", "mp4", "mov"], "default": "webm", "description": "Output container/codec: \"webm\" (VP9 — web-playable AND keeps transparency), \"mp4\" (H.264 — universal but has no alpha, so it needs a background color), or \"mov\" (ProRes 4444 with alpha for editors; very large files). Default: webm." },
                    "quality":     { "type": "integer", "minimum": 1, "maximum": 100, "default": 75, "description": "Encode quality 1-100 (higher = better and larger). Maps to the codec's CRF; ignored for mov/ProRes, which is always near-lossless. Default: 75." },
                    "keep_audio":  { "type": "boolean", "default": true, "description": "Keep the original audio track (true, default) or drop it (false). Silent clips are unaffected." }
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
    fn output_filename_uses_rounded_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-rounded", "webm"),
            "clip-rounded.webm"
        );
    }
}
