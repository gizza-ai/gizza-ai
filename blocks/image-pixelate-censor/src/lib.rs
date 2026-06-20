//! gizza-ai/image-pixelate-censor — censor a rectangular region of an image by
//! pixelating (mosaic) or blurring it. Returns a PNG.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::censor` (decode + region
//! pixelate/blur via the `image` crate) → PNG media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + image bytes output; the page
//! file-input path is ffmpeg-only — like add-text-to-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_pixelate_censor_core::{censor, parse_mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    strength: f64,
}
fn default_mode() -> String {
    "pixelate".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::integer("x").required().min(0.0).describe("Left edge of the region to censor (pixels from the left)."))
        .param(Param::integer("y").required().min(0.0).describe("Top edge of the region to censor (pixels from the top)."))
        .param(Param::integer("width").required().min(1.0).describe("Width of the region in pixels (clamped to the image)."))
        .param(Param::integer("height").required().min(1.0).describe("Height of the region in pixels (clamped to the image)."))
        .param(Param::enumv("mode", ["pixelate", "blur"]).default("pixelate").describe("Censor style: pixelate (mosaic, default) or blur."))
        .param(Param::number("strength").min(0.0).describe("Pixelate: mosaic tile size in px (default 16). Blur: blur radius/sigma (default 12). 0 = default."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImagePixelateCensor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-pixelate-censor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pixelate or blur a region of an image to censor it",
    requires = ["wafer-run/network"],
    skill(
        description = "Censor a rectangular region of an image (e.g. a face, license plate, or sensitive text) by pixelating it into a mosaic (default) or blurring it. Give the region as x, y, width, height in pixels (0-based from the top-left; clamped to the image). mode is 'pixelate' or 'blur'; strength sets the mosaic tile size (pixelate) or blur radius (blur). Only the region is changed; the rest is untouched. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ImagePixelateCensor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-pixelate-censor")?;
    let mode = parse_mode(&args.mode).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let png = censor(
        &bytes,
        args.x,
        args.y,
        args.width,
        args.height,
        mode,
        args.strength as f32,
    )
    .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "censored.png".to_string(),
        format!(
            "{} a {}x{} region at ({},{}) ({} bytes PNG)",
            args.mode, args.width, args.height, args.x, args.y, png.len()
        ),
        MAX_OUTPUT_BYTES,
    )
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
                    "url":      { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "x":        { "type": "integer", "minimum": 0, "description": "Left edge of the region to censor (pixels from the left)." },
                    "y":        { "type": "integer", "minimum": 0, "description": "Top edge of the region to censor (pixels from the top)." },
                    "width":    { "type": "integer", "minimum": 1, "description": "Width of the region in pixels (clamped to the image)." },
                    "height":   { "type": "integer", "minimum": 1, "description": "Height of the region in pixels (clamped to the image)." },
                    "mode":     { "type": "string", "enum": ["pixelate", "blur"], "default": "pixelate", "description": "Censor style: pixelate (mosaic, default) or blur." },
                    "strength": { "type": "number", "minimum": 0, "description": "Pixelate: mosaic tile size in px (default 16). Blur: blur radius/sigma (default 12). 0 = default." }
                },
                "additionalProperties": false,
                "required": ["x", "y", "width", "height"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
