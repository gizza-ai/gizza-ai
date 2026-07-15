//! gizza-ai/image-split-overlay — composite two images into one static
//! before/after split frame. Chat + CLI only (source-list input + image output;
//! no standalone page, matching image-collage/gif-from-images).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_split_overlay_core::{
    parse_color, parse_fit, parse_format, parse_orientation, split_overlay_from_bytes,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_position")]
    position: f64,
    #[serde(default = "default_fit")]
    fit: String,
    #[serde(default = "default_divider_width")]
    divider_width: u64,
    #[serde(default = "default_divider_color")]
    divider_color: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_orientation() -> String { "vertical".to_string() }
fn default_position() -> f64 { 50.0 }
fn default_fit() -> String { "stretch".to_string() }
fn default_divider_width() -> u64 { 4 }
fn default_divider_color() -> String { "#ffffff".to_string() }
fn default_format() -> String { "png".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): image A first, image B second. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv("orientation", ["vertical", "horizontal", "diagonal", "diagonal-reverse"])
                .default("vertical")
                .describe("Split geometry: vertical (A left/B right), horizontal (A top/B bottom), diagonal (top-left to bottom-right), or diagonal-reverse."),
        )
        .param(
            Param::number("position")
                .min(0.0)
                .max(100.0)
                .default(50.0)
                .describe("Split position as a percentage from 0 to 100. Default 50."),
        )
        .param(
            Param::enumv("fit", ["stretch", "cover"])
                .default("stretch")
                .describe("How image B is fitted to image A's canvas: stretch to exact size, or cover with center-crop. Default stretch."),
        )
        .param(
            Param::integer("divider_width")
                .min(0.0)
                .max(400.0)
                .default(4)
                .describe("Divider seam width in pixels, 0-400. Use 0 for no divider. Default 4."),
        )
        .param(
            Param::string("divider_color")
                .default("#ffffff")
                .describe("Divider color as #rgb, #rrggbb, or #rrggbbaa. Default white."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output image format: png (default, preserves alpha) or jpeg."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct ImageSplitOverlay;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-split-overlay",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Create a static before/after split overlay from two images.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Create a static before/after split overlay from exactly two image sources. Image A defines the output canvas; image B is fitted by stretch or cover. orientation chooses vertical, horizontal, diagonal, or diagonal-reverse; position is 0-100 percent; divider_width and divider_color draw the seam; format outputs png or jpeg. Provide images as an ordered list, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ImageSplitOverlay {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("image-split-overlay")?;
    if args.images.len() != 2 {
        return Err(SkillError::InvalidArgs(
            "image-split-overlay needs exactly 2 image sources (A then B)".into(),
        ));
    }
    let orientation = parse_orientation(&args.orientation).map_err(SkillError::InvalidArgs)?;
    let fit = parse_fit(&args.fit).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(&args.format).map_err(SkillError::InvalidArgs)?;
    let divider_color = parse_color(&args.divider_color).map_err(SkillError::InvalidArgs)?;
    let position = args.position.clamp(0.0, 100.0);
    let divider_width = args.divider_width.min(400) as u32;

    let mut resolved = Vec::with_capacity(2);
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        resolved.push(bytes);
    }

    let out = split_overlay_from_bytes(
        &resolved[0],
        &resolved[1],
        orientation,
        position,
        fit,
        divider_width,
        divider_color,
        format,
    )
    .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &out,
        format.mime(),
        format!("split-overlay.{}", format.ext()),
        format!(
            "created {} before/after split overlay at {:.1}% ({} bytes {})",
            args.orientation,
            position,
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
                        "description": "Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): image A first, image B second. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "orientation": { "type": "string", "enum": ["vertical", "horizontal", "diagonal", "diagonal-reverse"], "default": "vertical", "description": "Split geometry: vertical (A left/B right), horizontal (A top/B bottom), diagonal (top-left to bottom-right), or diagonal-reverse." },
                    "position": { "type": "number", "minimum": 0, "maximum": 100, "default": 50.0, "description": "Split position as a percentage from 0 to 100. Default 50." },
                    "fit": { "type": "string", "enum": ["stretch", "cover"], "default": "stretch", "description": "How image B is fitted to image A's canvas: stretch to exact size, or cover with center-crop. Default stretch." },
                    "divider_width": { "type": "integer", "minimum": 0, "maximum": 400, "default": 4, "description": "Divider seam width in pixels, 0-400. Use 0 for no divider. Default 4." },
                    "divider_color": { "type": "string", "default": "#ffffff", "description": "Divider color as #rgb, #rrggbb, or #rrggbbaa. Default white." },
                    "format": { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output image format: png (default, preserves alpha) or jpeg." }
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
