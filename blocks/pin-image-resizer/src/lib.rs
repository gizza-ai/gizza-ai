//! gizza-ai/pin-image-resizer — fetch an image URL or attachment ref, resize +
//! crop it to one of Pinterest's recommended pin formats via ffmpeg, and return
//! the media envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builders + the
//! preset table stay in `core`. See blocks/image-resize/src/lib.rs for the
//! full-example this mirrors.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_pin_image_resizer_core::{plan, preset_dims};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024; // Pinterest's own 20 MB image cap.
const MAX_OUTPUT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    fit: Option<String>,
    #[serde(default)]
    gravity: Option<String>,
    #[serde(default)]
    background: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below pins the derived schema to an authored literal.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("preset", ["standard", "square", "tall", "story"])
                .default("standard")
                .describe("Pinterest pin format: standard = 1000x1500 (2:3, recommended); square = 1000x1000 (1:1); tall = 1000x2100 (long/infographic pin); story = 1080x1920 (9:16 idea pin). Default standard."),
        )
        .param(
            Param::enumv("fit", ["cover", "contain", "stretch"])
                .default("cover")
                .describe("How the source fills the pin box: cover (default) scales up and crops the overflow (no distortion, no bars); contain scales down inside the box and pads the rest with 'background'; stretch forces the exact box, distorting if the ratio differs."),
        )
        .param(
            Param::enumv("gravity", ["center", "top", "bottom"])
                .default("center")
                .describe("Which part to keep when fit=cover crops the overflow: center (default), top, or bottom. Ignored for contain/stretch."),
        )
        .param(
            Param::string("background")
                .default("white")
                .describe("Padding colour used when fit=contain, as a short or long hex value (#fff, #ffffff) or a plain colour name (white, black). Default white. Ignored for cover/stretch."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PinImageResizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pin-image-resizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resize and crop an image to a Pinterest pin format (2:3 1000x1500, square, tall, or story).",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Resize and crop an image to one of Pinterest's recommended pin formats. preset is standard (1000x1500, 2:3 — the recommended pin), square (1000x1000), tall (1000x2100 long/infographic pin), or story (1080x1920, 9:16 idea pin); default standard. fit is cover (default — scale up and crop the overflow, no distortion), contain (scale down inside the box and pad with 'background'), or stretch. gravity (center/top/bottom) chooses which part cover keeps. background is the pad colour for contain (#fff/#ffffff hex or a colour name, default white). Output keeps the input format (JPG/PNG). Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl PinImageResizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pin-image-resizer")?;
    let preset = args.preset.as_deref().unwrap_or("standard");
    // Validate the preset up front for a clean error (plan() re-checks it too).
    let p = preset_dims(preset).map_err(SkillError::InvalidArgs)?;

    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");

    let background = args.background.as_deref().unwrap_or("white");
    let (argv, _out_name) = plan(
        &ffmpeg_in,
        preset,
        args.fit.as_deref(),
        args.gravity.as_deref(),
        background,
    )
    .map_err(SkillError::InvalidArgs)?;
    let ffmpeg_out = format!("out.{ext}");
    // Re-target the argv's out filename to the mime-derived extension so ffmpeg
    // encodes to the input format (plan() keyed the ext off "in.{ext}" already,
    // so they match — but pin the output name explicitly for clarity).
    let mut argv = argv;
    if let Some(last) = argv.last_mut() {
        *last = ffmpeg_out.clone();
    }

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, &format!("-pin-{}x{}", p.w, p.h), ext);
    let for_llm = format!(
        "resized {in_filename} to Pinterest {} pin ({}x{}, {output_size} bytes {mime})",
        p.name, p.w, p.h
    );
    build_media_envelope(&output, &mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":        { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "preset":     { "type": "string", "enum": ["standard","square","tall","story"], "default": "standard", "description": "Pinterest pin format: standard = 1000x1500 (2:3, recommended); square = 1000x1000 (1:1); tall = 1000x2100 (long/infographic pin); story = 1080x1920 (9:16 idea pin). Default standard." },
                    "fit":        { "type": "string", "enum": ["cover","contain","stretch"], "default": "cover", "description": "How the source fills the pin box: cover (default) scales up and crops the overflow (no distortion, no bars); contain scales down inside the box and pads the rest with 'background'; stretch forces the exact box, distorting if the ratio differs." },
                    "gravity":    { "type": "string", "enum": ["center","top","bottom"], "default": "center", "description": "Which part to keep when fit=cover crops the overflow: center (default), top, or bottom. Ignored for contain/stretch." },
                    "background": { "type": "string", "default": "white", "description": "Padding colour used when fit=contain, as a short or long hex value (#fff, #ffffff) or a plain colour name (white, black). Default white. Ignored for cover/stretch." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
