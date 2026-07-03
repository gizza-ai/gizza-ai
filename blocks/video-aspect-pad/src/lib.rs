//! gizza-ai/video-aspect-pad — letterbox/pillarbox a video to a target aspect
//! ratio with a chosen bar color, via ffmpeg.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Aspect/width/color
//! validation and the pure argv builder live in `core`, shared with the page.
//! NOTE: chat ffmpeg is non-functional — page + CLI are the working surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_aspect_pad_core::{canvas, plan};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    aspect: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    blur: bool,
    #[serde(default)]
    quality: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("aspect", ["9:16", "1:1", "16:9", "4:5", "3:4", "4:3", "3:2", "2:3", "21:9"])
                .default("9:16")
                .describe("Target aspect ratio (width:height). 9:16 Reels/Shorts/TikTok, 1:1 square feed, 16:9 YouTube, 4:5 Instagram portrait, 3:2 classic photo, 2:3 Pinterest, 21:9 cinematic. Default 9:16."),
        )
        .param(
            Param::integer("width")
                .min(16.0)
                .max(4096.0)
                .describe("Output width in pixels (even, 16-4096); height follows the aspect ratio. Omit for the platform-standard canvas (1080x1920 for 9:16, 1920x1080 for 16:9, ...)."),
        )
        .param(
            Param::string("color")
                .default("black")
                .describe("Bar color: a CSS color name (black, white, navy, ...) or hex like #1A2B3C or #f0f. Default black. Ignored when blur=true."),
        )
        .param(
            Param::boolean("blur")
                .default(false)
                .describe("Fill the bars with a blurred, zoomed copy of the video instead of a solid color (the TV-news portrait look). Default false."),
        )
        .param(
            Param::enumv("quality", ["high", "medium", "low"])
                .default("medium")
                .describe("Encoding quality: high (CRF 18, bigger file), medium (CRF 23, balanced), low (CRF 28, smaller file). Default medium."),
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
struct VideoAspectPad;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-aspect-pad",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Letterbox or pillarbox a video to a target aspect ratio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Letterbox or pillarbox a video to a target aspect ratio — e.g. pad a landscape clip to 9:16 for Reels/Shorts/TikTok or to 1:1 for a feed post. The whole frame is kept (scaled to fit, never cropped or stretched) and centered on the target canvas; the rest is filled with a solid bar color, or with a blurred zoomed copy of the video itself (blur=true). Output is exactly the platform-standard canvas for the chosen aspect (1080x1920 for 9:16, 1080x1080 for 1:1, 1920x1080 for 16:9, ...) unless width sets a custom size. Provide the video as either url (HTTP/HTTPS) or ref. Video re-encodes as H.264 at the chosen quality tier (CRF 18/23/28); audio is copied untouched; MP4/MOV get +faststart for instant social streaming. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAspectPad {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; aspect/width/color/quality validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-aspect-pad")?;
    let aspect = args.aspect.as_deref().unwrap_or("9:16");
    let color = args.color.as_deref().unwrap_or("black");
    let quality = args.quality.as_deref().unwrap_or("medium");

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates everything).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, aspect, args.width, color, args.blur, quality)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope; spell out the canvas the video was padded onto.
    let (w, h) = canvas(aspect, args.width).map_err(SkillError::InvalidArgs)?;
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let suffix = format!("-pad-{}", aspect.replace(':', "x"));
    let filename = filename_with_suffix(&in_filename, &suffix, out_ext);
    let bars = if args.blur { "blurred".to_string() } else { format!("{color} bars") };
    let for_llm = format!(
        "padded {in_filename} to {aspect} ({w}x{h}, {bars}, {quality} quality, {output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Video
    /// wording); integer bounds serialize as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "aspect":  { "type": "string", "enum": ["9:16", "1:1", "16:9", "4:5", "3:4", "4:3", "3:2", "2:3", "21:9"], "default": "9:16", "description": "Target aspect ratio (width:height). 9:16 Reels/Shorts/TikTok, 1:1 square feed, 16:9 YouTube, 4:5 Instagram portrait, 3:2 classic photo, 2:3 Pinterest, 21:9 cinematic. Default 9:16." },
                    "width":   { "type": "integer", "minimum": 16, "maximum": 4096, "description": "Output width in pixels (even, 16-4096); height follows the aspect ratio. Omit for the platform-standard canvas (1080x1920 for 9:16, 1920x1080 for 16:9, ...)." },
                    "color":   { "type": "string", "default": "black", "description": "Bar color: a CSS color name (black, white, navy, ...) or hex like #1A2B3C or #f0f. Default black. Ignored when blur=true." },
                    "blur":    { "type": "boolean", "default": false, "description": "Fill the bars with a blurred, zoomed copy of the video instead of a solid color (the TV-news portrait look). Default false." },
                    "quality": { "type": "string", "enum": ["high", "medium", "low"], "default": "medium", "description": "Encoding quality: high (CRF 18, bigger file), medium (CRF 23, balanced), low (CRF 28, smaller file). Default medium." }
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
    fn output_filename_carries_aspect_pad_suffix() {
        assert_eq!(
            filename_with_suffix("holiday.mp4", "-pad-9x16", "mp4"),
            "holiday-pad-9x16.mp4"
        );
        assert_eq!(
            filename_with_suffix("clip.webm", "-pad-1x1", "webm"),
            "clip-pad-1x1.webm"
        );
    }
}
