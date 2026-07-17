//! gizza-ai/image-composite — overlay one image onto another with position, scale,
//! opacity, and a Photoshop-style blend mode. Chat + CLI only (source-list input +
//! image output; no standalone page, matching image-split-overlay/image-collage).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_composite_core::{
    composite_from_bytes, parse_blend_mode, parse_flip, parse_format, parse_position,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_blend_mode")]
    blend_mode: String,
    #[serde(default = "default_opacity")]
    opacity: f64,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_position")]
    position: String,
    #[serde(default)]
    offset_x: i64,
    #[serde(default)]
    offset_y: i64,
    #[serde(default = "default_flip")]
    flip: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_blend_mode() -> String { "normal".to_string() }
fn default_opacity() -> f64 { 1.0 }
fn default_scale() -> f64 { 100.0 }
fn default_position() -> String { "center".to_string() }
fn default_flip() -> String { "none".to_string() }
fn default_format() -> String { "png".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the background (base) image first, the overlay (foreground) image second. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv(
                "blend_mode",
                [
                    "normal", "multiply", "screen", "overlay", "darken", "lighten", "hard-light",
                    "soft-light", "difference", "exclusion", "add",
                ],
            )
            .default("normal")
            .describe("How the overlay's pixels combine with the base: normal (default, plain layering), multiply/darken (darken), screen/lighten/add (lighten), overlay/hard-light/soft-light (contrast), difference/exclusion (invert)."),
        )
        .param(
            Param::number("opacity")
                .min(0.0)
                .max(1.0)
                .default(1.0)
                .describe("Overlay opacity from 0.0 (invisible) to 1.0 (fully applied). Default 1.0."),
        )
        .param(
            Param::number("scale")
                .min(1.0)
                .max(1000.0)
                .default(100.0)
                .describe("Overlay size as a percentage of its native dimensions, 1-1000. 100 keeps its original size; 50 halves it; 200 doubles it. Default 100."),
        )
        .param(
            Param::enumv(
                "position",
                [
                    "center", "top-left", "top", "top-right", "left", "right", "bottom-left",
                    "bottom", "bottom-right",
                ],
            )
            .default("center")
            .describe("Where the overlay is anchored on the base before offsets: center (default) or any edge/corner."),
        )
        .param(
            Param::integer("offset_x")
                .min(-10000.0)
                .max(10000.0)
                .default(0)
                .describe("Horizontal nudge in pixels from the anchor, positive = right, negative = left. Default 0."),
        )
        .param(
            Param::integer("offset_y")
                .min(-10000.0)
                .max(10000.0)
                .default(0)
                .describe("Vertical nudge in pixels from the anchor, positive = down, negative = up. Default 0."),
        )
        .param(
            Param::enumv("flip", ["none", "horizontal", "vertical", "both"])
                .default("none")
                .describe("Mirror the overlay before compositing: none (default), horizontal, vertical, or both."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output image format: png (default, preserves transparency) or jpeg (flattens transparency)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct ImageComposite;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-composite",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Overlay one image onto another with position, scale, opacity, and blend modes.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Overlay one image (the foreground) onto another (the background) into a single composite. The first image source is the base and defines the output canvas; the second is the overlay, scaled by `scale` percent, optionally flipped, and placed at `position` plus `offset_x`/`offset_y` pixels. `blend_mode` (normal, multiply, screen, overlay, darken, lighten, hard-light, soft-light, difference, exclusion, add) and `opacity` control how the two combine; `format` outputs png (keeps alpha) or jpeg. Provide images as an ordered list of two, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ImageComposite {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("image-composite")?;
    if args.images.len() != 2 {
        return Err(SkillError::InvalidArgs(
            "image-composite needs exactly 2 image sources (background first, overlay second)".into(),
        ));
    }
    let blend = parse_blend_mode(&args.blend_mode).map_err(SkillError::InvalidArgs)?;
    let position = parse_position(&args.position).map_err(SkillError::InvalidArgs)?;
    let flip = parse_flip(&args.flip).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(&args.format).map_err(SkillError::InvalidArgs)?;
    let opacity = args.opacity.clamp(0.0, 1.0);
    let scale = args.scale.clamp(1.0, 1000.0);
    let offset_x = args.offset_x.clamp(-10000, 10000);
    let offset_y = args.offset_y.clamp(-10000, 10000);

    let mut resolved = Vec::with_capacity(2);
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        resolved.push(bytes);
    }

    let out = composite_from_bytes(
        &resolved[0], &resolved[1], blend, opacity, scale, position, offset_x, offset_y, flip,
        format,
    )
    .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &out,
        format.mime(),
        format!("composite.{}", format.ext()),
        format!(
            "composited overlay ({} blend, opacity {:.2}, scale {:.0}%) onto base ({} bytes {})",
            args.blend_mode,
            opacity,
            scale,
            out.len(),
            format.ext()
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
            r##"{
                "type": "object",
                "properties": {
                    "images": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the background (base) image first, the overlay (foreground) image second. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "blend_mode": { "type": "string", "enum": ["normal", "multiply", "screen", "overlay", "darken", "lighten", "hard-light", "soft-light", "difference", "exclusion", "add"], "default": "normal", "description": "How the overlay's pixels combine with the base: normal (default, plain layering), multiply/darken (darken), screen/lighten/add (lighten), overlay/hard-light/soft-light (contrast), difference/exclusion (invert)." },
                    "opacity": { "type": "number", "minimum": 0, "maximum": 1, "default": 1.0, "description": "Overlay opacity from 0.0 (invisible) to 1.0 (fully applied). Default 1.0." },
                    "scale": { "type": "number", "minimum": 1, "maximum": 1000, "default": 100.0, "description": "Overlay size as a percentage of its native dimensions, 1-1000. 100 keeps its original size; 50 halves it; 200 doubles it. Default 100." },
                    "position": { "type": "string", "enum": ["center", "top-left", "top", "top-right", "left", "right", "bottom-left", "bottom", "bottom-right"], "default": "center", "description": "Where the overlay is anchored on the base before offsets: center (default) or any edge/corner." },
                    "offset_x": { "type": "integer", "minimum": -10000, "maximum": 10000, "default": 0, "description": "Horizontal nudge in pixels from the anchor, positive = right, negative = left. Default 0." },
                    "offset_y": { "type": "integer", "minimum": -10000, "maximum": 10000, "default": 0, "description": "Vertical nudge in pixels from the anchor, positive = down, negative = up. Default 0." },
                    "flip": { "type": "string", "enum": ["none", "horizontal", "vertical", "both"], "default": "none", "description": "Mirror the overlay before compositing: none (default), horizontal, vertical, or both." },
                    "format": { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output image format: png (default, preserves transparency) or jpeg (flattens transparency)." }
                },
                "required": ["images"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
