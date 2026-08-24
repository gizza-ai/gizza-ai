//! gizza-ai/still-to-clip — fetch a single still image (URL or attachment ref)
//! and hold it for a fixed duration as a static video clip via ffmpeg.
//!
//! Nothing moves: the image2 demuxer holds the picture and `-t` fixes the
//! length, so the result is a clean "hold" for a timeline. (Pan/zoom motion is
//! `video-ken-burns`' job.)
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
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_still_to_clip_core::{
    normalize_color, parse_format, plan, DEFAULT_BACKGROUND, DEFAULT_DURATION, DEFAULT_FIT,
    DEFAULT_FORMAT, DEFAULT_FPS, DEFAULT_HEIGHT, DEFAULT_QUALITY, DEFAULT_WIDTH, FITS, FORMATS,
    MAX_DIM,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024; // 12 MiB still image
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024; // 32 MiB clip

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    fit: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    fps: f64,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    quality: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("duration")
                .min(0.1)
                .max(60.0)
                .default(DEFAULT_DURATION)
                .describe("How long the still is held, in seconds, 0.1-60 (default 5)."),
        )
        .param(
            Param::number("width")
                .min(16.0)
                .max(MAX_DIM as f64)
                .default(DEFAULT_WIDTH)
                .describe(
                    "Output width in pixels, 16-3840 (default 1920; snapped to an even number). \
                     Ignored when fit is original.",
                ),
        )
        .param(
            Param::number("height")
                .min(16.0)
                .max(MAX_DIM as f64)
                .default(DEFAULT_HEIGHT)
                .describe(
                    "Output height in pixels, 16-3840 (default 1080; snapped to an even number). \
                     Ignored when fit is original.",
                ),
        )
        .param(Param::enumv("fit", FITS).default(DEFAULT_FIT).describe(
            "How the picture is placed in the frame: contain (the default) fits it whole and \
             pads the leftover with the background color, cover fills the frame and center-crops \
             the overflow, stretch forces the exact size and distorts the aspect ratio, and \
             original keeps the image's own size and ignores width/height.",
        ))
        .param(
            Param::string("background")
                .default(DEFAULT_BACKGROUND)
                .describe(
                    "Color of the padding bars when fit is contain: a name like black (the \
                     default), white or navy, or hex like #FFFFFF or 0x1A2B3C. Unused by the \
                     other fit modes.",
                ),
        )
        .param(
            Param::number("fps")
                .min(1.0)
                .max(60.0)
                .default(DEFAULT_FPS)
                .describe(
                    "Frames per second, 1-60 (default 30). A still needs no motion, so a low \
                     value like 10 makes a much smaller file.",
                ),
        )
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output container: mp4 (the default, H.264 — plays everywhere), webm (VP9, smaller \
             and web-native), or mov (H.264 in a QuickTime container for editors).",
        ))
        .param(
            Param::number("quality")
                .min(1.0)
                .max(100.0)
                .default(DEFAULT_QUALITY as f64)
                .describe(
                    "Encoding quality 1-100 (default 80), mapped onto the codec's CRF. Higher is \
                     better looking and larger; 80 is visually clean for a static hold.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve the effective params (0 / blank = unset → default) shared by run() +
/// tests, so every surface applies the same contract.
fn resolved(args: &Args) -> (f64, u32, u32, String, String, f64, String, u8) {
    let pick = |v: &Option<String>, d: &str| {
        v.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| d.to_string())
    };
    let duration = if args.duration > 0.0 { args.duration } else { DEFAULT_DURATION };
    let width = if args.width > 0.0 { args.width.round() as u32 } else { DEFAULT_WIDTH };
    let height = if args.height > 0.0 { args.height.round() as u32 } else { DEFAULT_HEIGHT };
    let fps = if args.fps > 0.0 { args.fps } else { DEFAULT_FPS };
    let quality = if args.quality > 0.0 {
        args.quality.round().clamp(0.0, 255.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    (
        duration,
        width,
        height,
        pick(&args.fit, DEFAULT_FIT),
        pick(&args.background, DEFAULT_BACKGROUND),
        fps,
        pick(&args.format, DEFAULT_FORMAT),
        quality,
    )
}

#[cfg(target_arch = "wasm32")]
struct StillToClip;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/still-to-clip",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a single still image into a fixed-duration static video clip at a chosen size, frame rate and format.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Turn a single still image into a fixed-duration static video clip — the picture is held motionless for the requested time (use video-ken-burns instead if you want pan/zoom motion). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Optional duration (seconds 0.1-60, default 5), width/height in pixels (default 1920x1080, snapped even), fit (contain [default] pads with the background color, cover center-crops, stretch distorts, original keeps the source size), background color for contain padding (default black), fps (1-60, default 30), format (mp4 [default, H.264], webm [VP9], mov), and quality (1-100, default 80).",
        parameters = schema_json()
    ),
)]
impl StillToClip {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid still-to-clip args: {e}")))?;
    let (duration, width, height, fit, background, fps, format, quality) = resolved(&args);
    let background = normalize_color(&background)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid still-to-clip args: {e}")))?;
    let fmt = parse_format(&format)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid still-to-clip args: {e}")))?;

    // 2. Resolve source — URL fetch or attachment lookup (a still image).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let in_ext = mime_to_ext(&in_mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {in_mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        duration, width, height, &fit, &background, fps, &format, quality, &ffmpeg_in,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid still-to-clip args: {e}")))?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope. Output is a video clip in the chosen container.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-clip", fmt.ext());
    let for_llm = format!(
        "held {in_filename} as a {duration:.1}s static clip ({fit}, {fps:.0} fps, {output_size} bytes {})",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":        { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "duration":   { "type": "number", "minimum": 0.1, "maximum": 60, "default": 5.0, "description": "How long the still is held, in seconds, 0.1-60 (default 5)." },
                    "width":      { "type": "number", "minimum": 16, "maximum": 3840, "default": 1920, "description": "Output width in pixels, 16-3840 (default 1920; snapped to an even number). Ignored when fit is original." },
                    "height":     { "type": "number", "minimum": 16, "maximum": 3840, "default": 1080, "description": "Output height in pixels, 16-3840 (default 1080; snapped to an even number). Ignored when fit is original." },
                    "fit":        { "type": "string", "enum": ["contain", "cover", "stretch", "original"], "default": "contain", "description": "How the picture is placed in the frame: contain (the default) fits it whole and pads the leftover with the background color, cover fills the frame and center-crops the overflow, stretch forces the exact size and distorts the aspect ratio, and original keeps the image's own size and ignores width/height." },
                    "background": { "type": "string", "default": "black", "description": "Color of the padding bars when fit is contain: a name like black (the default), white or navy, or hex like #FFFFFF or 0x1A2B3C. Unused by the other fit modes." },
                    "fps":        { "type": "number", "minimum": 1, "maximum": 60, "default": 30.0, "description": "Frames per second, 1-60 (default 30). A still needs no motion, so a low value like 10 makes a much smaller file." },
                    "format":     { "type": "string", "enum": ["mp4", "webm", "mov"], "default": "mp4", "description": "Output container: mp4 (the default, H.264 — plays everywhere), webm (VP9, smaller and web-native), or mov (H.264 in a QuickTime container for editors)." },
                    "quality":    { "type": "number", "minimum": 1, "maximum": 100, "default": 80.0, "description": "Encoding quality 1-100 (default 80), mapped onto the codec's CRF. Higher is better looking and larger; 80 is visually clean for a static hold." }
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
        let (dur, w, h, fit, bg, fps, fmt, q) = resolved(&args);
        assert_eq!((dur, w, h, fps, q), (5.0, 1920, 1080, 30.0, 80));
        assert_eq!((fit.as_str(), bg.as_str(), fmt.as_str()), ("contain", "black", "mp4"));
    }

    #[test]
    fn resolved_passes_through_explicit_values() {
        let args: Args = serde_json::from_str(
            r##"{"ref":"img-1","duration":2.5,"width":1080,"height":1080,"fit":"cover","background":"#FFFFFF","fps":24,"format":"webm","quality":60}"##,
        )
        .unwrap();
        let (dur, w, h, fit, bg, fps, fmt, q) = resolved(&args);
        assert_eq!((dur, w, h, fps, q), (2.5, 1080, 1080, 24.0, 60));
        assert_eq!((fit.as_str(), bg.as_str(), fmt.as_str()), ("cover", "#FFFFFF", "webm"));
    }

    /// Blank strings (what an empty chat/CLI field sends) mean "default", not
    /// "empty" — an empty background would otherwise fail core validation.
    #[test]
    fn blank_strings_fall_back_to_defaults() {
        let args: Args = serde_json::from_str(
            r#"{"url":"http://x/i.png","fit":"","background":"  ","format":""}"#,
        )
        .unwrap();
        let (_, _, _, fit, bg, _, fmt, _) = resolved(&args);
        assert_eq!((fit.as_str(), bg.as_str(), fmt.as_str()), ("contain", "black", "mp4"));
    }

    /// The resolved defaults must survive the shared core's validation, and the
    /// output name must follow the chosen container on every surface.
    #[test]
    fn resolved_defaults_build_a_valid_plan_and_filename() {
        let args: Args = serde_json::from_str(r#"{"url":"http://x/photo.jpg"}"#).unwrap();
        let (dur, w, h, fit, bg, fps, fmt, q) = resolved(&args);
        let bg = normalize_color(&bg).unwrap();
        let (argv, out) = plan(dur, w, h, &fit, &bg, fps, &fmt, q, "in.jpg").expect("valid plan");
        assert_eq!(out, "out.mp4");
        assert_eq!(argv[argv.iter().position(|a| a == "-t").unwrap() + 1], "5");
        assert_eq!(
            filename_with_suffix("photo.jpg", "-clip", parse_format(&fmt).unwrap().ext()),
            "photo-clip.mp4"
        );
        assert_eq!(
            filename_with_suffix("photo.jpg", "-clip", parse_format("webm").unwrap().ext()),
            "photo-clip.webm"
        );
    }
}
