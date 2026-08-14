//! gizza-ai/image-auto-orient — rotate a photo upright using its EXIF
//! `Orientation` flag and BAKE the rotation into the pixels, so viewers that
//! ignore the flag still show it the right way up.
//!
//! ffmpeg applies EXIF autorotation by default, so `orientation = "auto"` is a
//! plain transcode; `orientation = "1".."8"` forces a specific correction with
//! `-noautorotate` + an explicit filter chain (see the core's table). Either way
//! the output carries no orientation tag, so it can never be double-rotated.
//!
//! The chat schema is derived from `descriptor()` (single source — chat + CLI +
//! page); the handler delegates source resolution, ffmpeg dispatch, and
//! envelope-building to `block_utils`, and the pure `core::plan` argv builder is
//! shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, format_to_mime_and_ext, mime_to_ext, AssetKind,
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_auto_orient_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_orientation")]
    orientation: String,
    #[serde(default = "d_format")]
    format: String,
    #[serde(default = "d_quality")]
    quality: u8,
}

fn d_orientation() -> String {
    "auto".to_string()
}
fn d_format() -> String {
    "same".to_string()
}
fn d_quality() -> u8 {
    90
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv(
                "orientation",
                ["auto", "1", "2", "3", "4", "5", "6", "7", "8"],
            )
            .default("auto")
            .describe(
                "Which correction to bake in. auto (default) reads the photo's EXIF Orientation tag and applies it. Pass an EXIF value 1-8 to force a correction when the tag is missing, wrong, or already stripped: 1 already upright, 2 mirror left-right, 3 rotate 180, 4 mirror top-bottom, 5 mirror + rotate 90 clockwise, 6 rotate 90 clockwise, 7 mirror + rotate 90 counter-clockwise, 8 rotate 90 counter-clockwise.",
            ),
        )
        .param(
            Param::enumv("format", ["same", "jpeg", "png", "webp"])
                .default("same")
                .describe(
                    "Output format: same (default) keeps the input's format; jpeg, png, or webp re-encodes to that format.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(90)
                .describe(
                    "Output quality 1-100 for JPEG and WebP output (default 90; higher = better and larger). Ignored for PNG, which is lossless.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageAutoOrient;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-auto-orient",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-rotate a photo upright from its EXIF orientation flag and bake it into the pixels",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Rotate a photo to its correct upright orientation using the EXIF Orientation flag and bake the rotation into the pixels, so apps that ignore EXIF still show it upright. The output carries no orientation tag, so it cannot be double-rotated. orientation=auto (default) uses the file's own flag; orientation=1-8 forces an EXIF correction when the flag is missing or wrong. format=same|jpeg|png|webp (default same) and quality=1-100 (default 90, JPEG/WebP only) control the output file. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageAutoOrient {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("image-auto-orient")?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let in_ext = mime_to_ext(&in_mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {in_mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&args.orientation, &args.format, args.quality, &ffmpeg_in)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the output mime/extension follow the chosen format.
    let (out_mime, out_ext) = match args.format.as_str() {
        "same" => (in_mime.as_str(), in_ext),
        other => format_to_mime_and_ext(AssetKind::Image, other).ok_or_else(|| {
            SkillError::InvalidArgs(format!("format {other:?} not supported (same|jpeg|png|webp)"))
        })?,
    };
    let filename = filename_with_suffix(&in_filename, "-oriented", out_ext);
    let applied = if args.orientation == "auto" {
        "its EXIF orientation flag".to_string()
    } else {
        format!("forced EXIF orientation {}", args.orientation)
    };
    let for_llm = format!(
        "auto-oriented {in_filename} using {applied}; rotation baked into the pixels and the orientation tag dropped ({} bytes, {out_mime})",
        output.len()
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing surface can't change by accident.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "orientation": { "type": "string", "enum": ["auto","1","2","3","4","5","6","7","8"], "default": "auto", "description": "Which correction to bake in. auto (default) reads the photo's EXIF Orientation tag and applies it. Pass an EXIF value 1-8 to force a correction when the tag is missing, wrong, or already stripped: 1 already upright, 2 mirror left-right, 3 rotate 180, 4 mirror top-bottom, 5 mirror + rotate 90 clockwise, 6 rotate 90 clockwise, 7 mirror + rotate 90 counter-clockwise, 8 rotate 90 counter-clockwise." },
                    "format":      { "type": "string", "enum": ["same","jpeg","png","webp"], "default": "same", "description": "Output format: same (default) keeps the input's format; jpeg, png, or webp re-encodes to that format." },
                    "quality":     { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "Output quality 1-100 for JPEG and WebP output (default 90; higher = better and larger). Ignored for PNG, which is lossless." }
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
    fn output_filename_gets_the_oriented_suffix() {
        assert_eq!(
            filename_with_suffix("IMG_4821.jpg", "-oriented", "jpg"),
            "IMG_4821-oriented.jpg"
        );
        // A format change also changes the extension.
        assert_eq!(
            filename_with_suffix("IMG_4821.jpg", "-oriented", "png"),
            "IMG_4821-oriented.png"
        );
    }

    #[test]
    fn format_values_all_map_to_a_mime_and_extension() {
        for (fmt, mime, ext) in [
            ("jpeg", "image/jpeg", "jpg"),
            ("png", "image/png", "png"),
            ("webp", "image/webp", "webp"),
        ] {
            assert_eq!(
                format_to_mime_and_ext(AssetKind::Image, fmt),
                Some((mime, ext)),
                "format {fmt}"
            );
        }
    }
}
