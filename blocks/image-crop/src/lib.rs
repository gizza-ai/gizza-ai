//! gizza-ai/image-crop — fetch an image URL or attachment ref, crop a rectangle
//! via ffmpeg, return envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (width/height must be > 0) and the pure `core` argv builder stay here. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard + unit tests below
// can exercise them. See image-resize for the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_crop_core::build_argv;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("x")
                .required()
                .min(0.0)
                .describe("Left offset of the crop rectangle in pixels."),
        )
        .param(
            Param::integer("y")
                .required()
                .min(0.0)
                .describe("Top offset of the crop rectangle in pixels."),
        )
        .param(
            Param::integer("width")
                .required()
                .min(1.0)
                .describe("Width of the crop rectangle in pixels."),
        )
        .param(
            Param::integer("height")
                .required()
                .min(1.0)
                .describe("Height of the crop rectangle in pixels."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageCrop;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-crop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crop a rectangular region from an image fetched by URL or from a prior tool call ref",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Crop a rectangular region from an image. Coordinates are in pixels with (0,0) at the top-left corner. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl ImageCrop {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — width/height must be > 0).
    let args: Args = serde_json::from_slice(&body).invalid_args("image-crop")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid image-crop args: width and height must be > 0".into(),
        ));
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let ffmpeg_out = format!("out.{ext}");
    let argv = build_argv(
        &ffmpeg_in,
        &ffmpeg_out,
        args.x,
        args.y,
        args.width,
        args.height,
    );

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-cropped", ext);
    let for_llm = format!(
        "cropped {} at ({},{}) {}x{} ({} {})",
        in_filename, args.x, args.y, args.width, args.height, output_size, mime
    );
    build_media_envelope(&output, &mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// emits `additionalProperties: false` uniformly (image-crop's authored
    /// schema lacked it — added below as intentional uniform hardening) and the
    /// shared `url`/`ref` property descriptions are centralized in
    /// `to_schema_json`, so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "x":      { "type": "integer", "minimum": 0, "description": "Left offset of the crop rectangle in pixels." },
                    "y":      { "type": "integer", "minimum": 0, "description": "Top offset of the crop rectangle in pixels." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Width of the crop rectangle in pixels." },
                    "height": { "type": "integer", "minimum": 1, "description": "Height of the crop rectangle in pixels." }
                },
                "additionalProperties": false,
                "required": ["x", "y", "width", "height"],
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
    fn output_filename_appends_cropped_suffix() {
        assert_eq!(
            filename_with_suffix("cat.png", "-cropped", "png"),
            "cat-cropped.png"
        );
    }
}
