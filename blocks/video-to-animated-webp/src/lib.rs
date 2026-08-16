//! gizza-ai/video-to-animated-webp — convert a video clip into looping animated
//! WebP via ffmpeg-runtime. Chat/CLI/page share the descriptor and the pure core
//! argv builder.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
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
    #[serde(default)]
    quality: f64,
    #[serde(default)]
    lossless: bool,
}

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
                .describe("Clip length in seconds (default 0 = to the end of the video). Keep animated WebP clips short for browser compatibility and file size."),
        )
        .param(
            Param::number("fps")
                .min(0.0)
                .max(60.0)
                .describe("Frames per second for the animated WebP (default 12). Lower fps = smaller file."),
        )
        .param(
            Param::number("width")
                .min(0.0)
                .max(4096.0)
                .describe("Output width in pixels; height scales to keep aspect ratio and is rounded even (default 0 = keep source size)."),
        )
        .param(
            Param::number("quality")
                .min(0.0)
                .max(100.0)
                .describe("Lossy WebP quality 0-100 (default 80). Ignored when lossless is true."),
        )
        .param(
            Param::boolean("lossless")
                .default(false)
                .describe("Use lossless WebP encoding instead of lossy quality mode (default false). Preserves detail and transparency, but files are usually larger."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToAnimatedWebp;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-animated-webp",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a short video clip into a looping animated WebP",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a video clip into a looping animated WebP, a modern smaller alternative to GIF that can preserve transparency. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Optional start (seconds, default 0), duration (seconds, default 0 = to the end), fps (default 12, max 60), width (default 0 = keep source size), quality (lossy WebP quality 0-100, default 80), and lossless (default false). The output is image/webp encoded by ffmpeg/libwebp with loop=0. Keep clips short: animated WebP is efficient, but long/high-fps clips can still be large.",
        parameters = schema_json()
    ),
)]
impl VideoToAnimatedWebp {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-animated-webp args: {e}")))?;
    gizza_ai_video_to_animated_webp_core::plan(
        "in.mp4",
        args.start,
        args.duration,
        args.fps,
        args.width,
        args.quality,
        args.lossless,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-animated-webp args: {e}")))?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).unwrap_or("mp4");
    let in_file = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_video_to_animated_webp_core::plan(
        &in_file,
        args.start,
        args.duration,
        args.fps,
        args.width,
        args.quality,
        args.lossless,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_file, bytes, out_name)?;
    let filename = filename_with_suffix(&in_name, "", "webp");
    let mode = if args.lossless { "lossless" } else { "lossy" };
    let for_llm = format!(
        "made a looping animated WebP from {in_name} ({} bytes, {mode})",
        output.len()
    );
    build_media_envelope(&output, "image/webp", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "start":    { "type": "number", "minimum": 0, "description": "Start time in seconds (default 0, from the beginning)." },
                    "duration": { "type": "number", "minimum": 0, "description": "Clip length in seconds (default 0 = to the end of the video). Keep animated WebP clips short for browser compatibility and file size." },
                    "fps":      { "type": "number", "minimum": 0, "maximum": 60, "description": "Frames per second for the animated WebP (default 12). Lower fps = smaller file." },
                    "width":    { "type": "number", "minimum": 0, "maximum": 4096, "description": "Output width in pixels; height scales to keep aspect ratio and is rounded even (default 0 = keep source size)." },
                    "quality":  { "type": "number", "minimum": 0, "maximum": 100, "description": "Lossy WebP quality 0-100 (default 80). Ignored when lossless is true." },
                    "lossless": { "type": "boolean", "default": false, "description": "Use lossless WebP encoding instead of lossy quality mode (default false). Preserves detail and transparency, but files are usually larger." }
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
    fn output_filename_uses_webp_ext() {
        assert_eq!(filename_with_suffix("clip.mp4", "", "webp"), "clip.webp");
    }
}
