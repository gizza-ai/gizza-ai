//! gizza-ai/youtube-thumbnail-generator — ffmpeg chat skill.
//!
//! Extracts one video frame and renders a thumbnail PNG with a bundled font,
//! outlined headline text, and an accent bar. The standalone page and CLI are the
//! primary verified surfaces; chat ffmpeg is not available in the Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_inputs, resolve_source};
use gizza_ai_youtube_thumbnail_generator_core::{plan, FONT_BYTES, FONT_FILE, TEXT_FILE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    headline: String,
    #[serde(default)]
    timestamp: Option<f64>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    text_position: Option<String>,
    #[serde(default)]
    font_size: Option<f64>,
    #[serde(default)]
    text_color: Option<String>,
    #[serde(default)]
    outline_color: Option<String>,
    #[serde(default)]
    accent_color: Option<String>,
    #[serde(default)]
    accent_position: Option<String>,
    #[serde(default)]
    accent_size: Option<f64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::string("headline")
                .required()
                .describe("Large headline text to draw on the thumbnail. Drawn literally from a text file, so punctuation is safe. Keep it short (max 120 characters)."),
        )
        .param(
            Param::number("timestamp")
                .min(0.0)
                .default(0.0)
                .describe("Seconds into the video to capture the source frame. Default 0."),
        )
        .param(
            Param::integer("width")
                .min(320.0)
                .max(4096.0)
                .default(1280)
                .describe("Output thumbnail width in pixels. Default 1280 (YouTube 16:9)."),
        )
        .param(
            Param::integer("height")
                .min(180.0)
                .max(4096.0)
                .default(720)
                .describe("Output thumbnail height in pixels. Default 720 (YouTube 16:9)."),
        )
        .param(
            Param::enumv("text_position", ["bottom", "top", "center"])
                .default("bottom")
                .describe("Where to anchor the outlined headline: bottom, top, or center. Default bottom."),
        )
        .param(
            Param::integer("font_size")
                .min(16.0)
                .max(220.0)
                .default(96)
                .describe("Headline font size in pixels (16-220). Default 96."),
        )
        .param(
            Param::string("text_color")
                .default("#ffffff")
                .describe("Headline fill color as a CSS color name or hex (#ffffff, #fc0). Default white."),
        )
        .param(
            Param::string("outline_color")
                .default("#000000")
                .describe("Headline outline color as a CSS color name or hex. Default black."),
        )
        .param(
            Param::string("accent_color")
                .default("#ffcc00")
                .describe("Accent bar color as a CSS color name or hex. Default #ffcc00."),
        )
        .param(
            Param::enumv("accent_position", ["bottom", "top", "left", "right", "none"])
                .default("bottom")
                .describe("Accent bar placement: bottom, top, left, right, or none. Default bottom."),
        )
        .param(
            Param::integer("accent_size")
                .min(0.0)
                .max(1024.0)
                .default(22)
                .describe("Accent bar thickness in pixels. Use 0 or accent_position=none to hide it. Default 22."),
        )
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/youtube-thumbnail-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a thumbnail PNG from a video frame with outlined headline text",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Generate a YouTube-style thumbnail PNG from a single video frame. Provide a video by url or ref, a headline, optional timestamp, output width/height (default 1280x720), headline position, font size, text and outline colors, and an accent bar color/position/size. The frame is scaled/cropped to the target canvas, a colored accent bar is drawn, and the headline is rendered with a bundled bold font via ffmpeg drawtext using textfile/fontfile (no inline text escaping). Returns a PNG image. Note: runs on the standalone page and CLI; chat ffmpeg is unavailable.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("youtube-thumbnail-generator")?;
    let (input_bytes, _in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ffmpeg_in = "in.mp4".to_string();
    let (argv, out_name) = plan(
        &ffmpeg_in,
        &args.headline,
        args.timestamp.unwrap_or(0.0),
        args.width.unwrap_or(1280.0),
        args.height.unwrap_or(720.0),
        args.text_position.as_deref().unwrap_or("bottom"),
        args.font_size.unwrap_or(96.0),
        args.text_color.as_deref().unwrap_or("#ffffff"),
        args.outline_color.as_deref().unwrap_or("#000000"),
        args.accent_color.as_deref().unwrap_or("#ffcc00"),
        args.accent_position.as_deref().unwrap_or("bottom"),
        args.accent_size.unwrap_or(22.0),
    )
    .map_err(SkillError::InvalidArgs)?;
    let inputs = vec![
        (ffmpeg_in.clone(), input_bytes),
        (FONT_FILE.to_string(), FONT_BYTES.to_vec()),
        (TEXT_FILE.to_string(), args.headline.clone().into_bytes()),
    ];
    let output = dispatch_ffmpeg_inputs(argv, inputs, out_name)?;
    let filename = filename_with_suffix(&in_filename, "-thumbnail", "png");
    let for_llm = format!("generated thumbnail PNG from {in_filename} with headline \"{}\"", args.headline);
    build_media_envelope(&output, "image/png", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "headline": { "type": "string", "description": "Large headline text to draw on the thumbnail. Drawn literally from a text file, so punctuation is safe. Keep it short (max 120 characters)." },
                    "timestamp": { "type": "number", "minimum": 0, "default": 0.0, "description": "Seconds into the video to capture the source frame. Default 0." },
                    "width": { "type": "integer", "minimum": 320, "maximum": 4096, "default": 1280, "description": "Output thumbnail width in pixels. Default 1280 (YouTube 16:9)." },
                    "height": { "type": "integer", "minimum": 180, "maximum": 4096, "default": 720, "description": "Output thumbnail height in pixels. Default 720 (YouTube 16:9)." },
                    "text_position": { "type": "string", "enum": ["bottom", "top", "center"], "default": "bottom", "description": "Where to anchor the outlined headline: bottom, top, or center. Default bottom." },
                    "font_size": { "type": "integer", "minimum": 16, "maximum": 220, "default": 96, "description": "Headline font size in pixels (16-220). Default 96." },
                    "text_color": { "type": "string", "default": "#ffffff", "description": "Headline fill color as a CSS color name or hex (#ffffff, #fc0). Default white." },
                    "outline_color": { "type": "string", "default": "#000000", "description": "Headline outline color as a CSS color name or hex. Default black." },
                    "accent_color": { "type": "string", "default": "#ffcc00", "description": "Accent bar color as a CSS color name or hex. Default #ffcc00." },
                    "accent_position": { "type": "string", "enum": ["bottom", "top", "left", "right", "none"], "default": "bottom", "description": "Accent bar placement: bottom, top, left, right, or none. Default bottom." },
                    "accent_size": { "type": "integer", "minimum": 0, "maximum": 1024, "default": 22, "description": "Accent bar thickness in pixels. Use 0 or accent_position=none to hide it. Default 22." }
                },
                "required": ["headline"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_thumbnail_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-thumbnail", "png"), "clip-thumbnail.png");
    }
}
