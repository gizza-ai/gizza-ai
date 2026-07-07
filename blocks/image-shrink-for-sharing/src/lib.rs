//! gizza-ai/image-shrink-for-sharing — fetch an image URL/ref, downscale + strip
//! metadata + re-encode (optionally converting format) via ffmpeg in one pass.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation +
//! the pure `core` argv builder stay here. See blocks/image-resize/src/lib.rs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_shrink_for_sharing_core::{format_from_name, plan_shrink, resolve_out_format};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

fn default_max_dimension() -> u32 {
    1600
}
fn default_quality() -> u8 {
    80
}
fn default_format() -> String {
    "keep".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_max_dimension")]
    max_dimension: u32,
    #[serde(default = "default_quality")]
    quality: u8,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    strip_metadata: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("max_dimension")
                .min(0.0)
                .default(1600)
                .describe(
                    "Cap the longest side to this many pixels; aspect ratio is kept and the image is never upscaled. 0 keeps the original size. Default 1600.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(80)
                .describe("Re-encode quality from 1 (smallest file) to 100 (best looking). Default 80."),
        )
        .param(
            Param::enumv("format", ["keep", "jpeg", "png", "webp"])
                .default("keep")
                .describe(
                    "Output format: keep uses the input's format; jpeg and webp shrink photos most; png suits flat graphics/screenshots. Default keep.",
                ),
        )
        .param(
            Param::boolean("strip_metadata")
                .default(true)
                .describe("Remove EXIF, GPS location and other metadata for privacy. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageShrinkForSharing;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-shrink-for-sharing",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Downscale, strip metadata, and compress an image for sharing in one step.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Shrink an image for messaging or upload: downscale the longest side, strip EXIF/GPS metadata, and re-encode at a chosen quality (optionally converting format) in one pass. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Supports jpg/jpeg, png, webp.",
        parameters = schema_json()
    ),
)]
impl ImageShrinkForSharing {
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
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid image-shrink-for-sharing args: {e}"))
    })?;
    if !(1..=100).contains(&args.quality) {
        return Err(SkillError::InvalidArgs(format!(
            "invalid image-shrink-for-sharing args: quality must be 1-100, got {}",
            args.quality
        )));
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Errors (unsupported input format,
    //    bad target format) surface here gracefully.
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let in_fmt = format_from_name(&ffmpeg_in).map_err(SkillError::InvalidArgs)?;
    let out_fmt = resolve_out_format(&args.format, in_fmt).map_err(SkillError::InvalidArgs)?;
    let (argv, ffmpeg_out) = plan_shrink(
        args.max_dimension,
        args.quality,
        &args.format,
        args.strip_metadata,
        &ffmpeg_in,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — output MIME/filename follow the OUTPUT format (may differ
    //    from the input when converting).
    let out_mime = out_fmt.mime();
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-shrunk", out_fmt.ext());
    let for_llm = format!(
        "shrank {in_filename} for sharing ({output_size} bytes, {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptor-derived chat schema must match this authored schema so the
    /// LLM/CLI/page all see a stable contract. Regenerate this literal (never keep
    /// a stale one) whenever the descriptor intentionally changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "max_dimension": { "type": "integer", "minimum": 0, "default": 1600, "description": "Cap the longest side to this many pixels; aspect ratio is kept and the image is never upscaled. 0 keeps the original size. Default 1600." },
                    "quality":  { "type": "integer", "minimum": 1, "maximum": 100, "default": 80, "description": "Re-encode quality from 1 (smallest file) to 100 (best looking). Default 80." },
                    "format":   { "type": "string", "enum": ["keep", "jpeg", "png", "webp"], "default": "keep", "description": "Output format: keep uses the input's format; jpeg and webp shrink photos most; png suits flat graphics/screenshots. Default keep." },
                    "strip_metadata": { "type": "boolean", "default": true, "description": "Remove EXIF, GPS location and other metadata for privacy. Default true." }
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
    fn output_filename_uses_shrunk_suffix_and_out_ext() {
        assert_eq!(
            filename_with_suffix("beach.png", "-shrunk", "jpg"),
            "beach-shrunk.jpg"
        );
    }
}
