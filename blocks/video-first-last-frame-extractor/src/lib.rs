//! gizza-ai/video-first-last-frame-extractor — grab a video's first and last frame
//! and stitch them into one comparison image (side by side or stacked), via ffmpeg.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Layout/format validation and
//! the pure argv builder live in `core`, shared with the page. The input is a
//! video but the output is always a single image (the stitched first+last frame),
//! so the page is `format="image"`. NOTE: chat ffmpeg is non-functional — the page
//! + CLI are the working surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_first_last_frame_extractor_core::{format_ext, plan};
use serde::Deserialize;
use wafer_sdk::*;

// `reverse` buffers the whole decoded video in RAM to reach the last frame, so
// keep the input modest — this is a "grab two keyframes" tool, not a transcoder.
const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

// Defaults — one source of truth for the handler fallbacks (the descriptor
// carries the same values for the chat/page-facing schema).
const DEFAULT_LAYOUT: &str = "horizontal";
const DEFAULT_FORMAT: &str = "png";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("layout", ["horizontal", "vertical"])
                .default(DEFAULT_LAYOUT)
                .describe("How the two frames are joined: horizontal = first frame left, last frame right (side by side); vertical = first on top, last on the bottom. Default horizontal."),
        )
        .param(
            Param::enumv("format", ["png", "jpg"])
                .default(DEFAULT_FORMAT)
                .describe("Output image format: png (lossless, crisp) or jpg (smaller file). Default png."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn out_mime(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        _ => "image/png",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoFirstLastFrameExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-first-last-frame-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Grab a video's first and last frame stitched into one comparison image",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Extract the first frame and the last frame of a video and stitch them into a single comparison image — side by side (horizontal) or stacked (vertical). Both frames are grabbed in one decode pass, so no timestamp is needed. Handy for spotting a clip's start-vs-end change, making a before/after thumbnail, or picking start/end keyframes for AI video tools. Output is one PNG (or JPG) image with the two frames joined; the source dimensions are preserved. Provide the video as either url (HTTP/HTTPS) or ref from a prior tool call. Note: chat ffmpeg is unavailable — runs on the standalone page and the CLI.",
        parameters = schema_json()
    ),
)]
impl VideoFirstLastFrameExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; layout/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid video-first-last-frame-extractor args: {e}"))
    })?;
    let layout = args.layout.as_deref().unwrap_or(DEFAULT_LAYOUT);
    let format = args.format.as_deref().unwrap_or(DEFAULT_FORMAT);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates layout + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, layout, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope — the stitched pair is a single image (PNG or JPEG).
    let out_ext = format_ext(format).map_err(SkillError::InvalidArgs)?;
    let mime = out_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-first-last", out_ext);
    let for_llm =
        format!("first + last frame of {in_filename} joined {layout}ly: {output_size} bytes {mime}");
    build_media_envelope(&output, mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "layout": { "type": "string", "enum": ["horizontal", "vertical"], "default": "horizontal", "description": "How the two frames are joined: horizontal = first frame left, last frame right (side by side); vertical = first on top, last on the bottom. Default horizontal." },
                    "format": { "type": "string", "enum": ["png", "jpg"], "default": "png", "description": "Output image format: png (lossless, crisp) or jpg (smaller file). Default png." }
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
    fn output_filename_carries_first_last_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-first-last", "png"),
            "clip-first-last.png"
        );
        assert_eq!(
            filename_with_suffix("holiday.webm", "-first-last", "jpg"),
            "holiday-first-last.jpg"
        );
    }
}
