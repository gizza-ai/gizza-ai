//! gizza-ai/image-collage — combine several images into one collage (grid,
//! horizontal strip, or vertical stack), returned as a PNG.
//!
//! Pipeline: resolve each image source (URL/ref) → pure `core::collage_from_bytes`
//! (decode + scale-to-fit + composite via the `image` crate) → PNG media
//! envelope. `Input::None` + a required `images` source_list (like images-to-pdf).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (array input + image bytes output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_collage_core::{collage_from_bytes, parse_color, parse_layout};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default)]
    columns: Option<u64>,
    #[serde(default)]
    gap: u64,
    #[serde(default = "default_bg")]
    background: String,
}
fn default_layout() -> String {
    "grid".to_string()
}
fn default_bg() -> String {
    "#ffffff".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 1)
                .required()
                .describe("Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv("layout", ["grid", "horizontal", "vertical"])
                .default("grid")
                .describe("Arrangement: grid (default), horizontal (one row), or vertical (one column)."),
        )
        .param(
            Param::integer("columns")
                .min(1.0)
                .describe("Number of columns for grid layout (default: roughly square). Ignored for horizontal/vertical."),
        )
        .param(
            Param::integer("gap")
                .min(0.0)
                .describe("Gap/padding in pixels between and around cells (default 0)."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background color as #rgb, #rrggbb, or #rrggbbaa (default white). Shows in gaps and letterbox areas."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageCollage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-collage",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine images into a collage (grid/strip/stack)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Combine several images into a single collage PNG. layout is grid (default), horizontal (one row), or vertical (one column); columns sets the grid width; gap adds pixel spacing; background sets the fill color (#rrggbb). Each image is scaled to fit a uniform cell with its aspect ratio preserved. Provide images as a list, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ImageCollage {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("image-collage")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs("image-collage needs at least 1 image".into()));
    }
    let layout = parse_layout(&args.layout).map_err(SkillError::InvalidArgs)?;
    let bg = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;
    let columns = args.columns.map(|c| c as usize);
    let gap = args.gap.min(200) as u32;

    let mut imgs: Vec<Vec<u8>> = Vec::with_capacity(args.images.len());
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        imgs.push(bytes);
    }
    let count = imgs.len();

    let png = collage_from_bytes(&imgs, layout, columns, gap, bg).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "collage.png".to_string(),
        format!("combined {count} images into a {} collage ({} bytes PNG)", args.layout, png.len()),
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
                        "minItems": 1,
                        "description": "Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "layout":     { "type": "string", "enum": ["grid", "horizontal", "vertical"], "default": "grid", "description": "Arrangement: grid (default), horizontal (one row), or vertical (one column)." },
                    "columns":    { "type": "integer", "minimum": 1, "description": "Number of columns for grid layout (default: roughly square). Ignored for horizontal/vertical." },
                    "gap":        { "type": "integer", "minimum": 0, "description": "Gap/padding in pixels between and around cells (default 0)." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background color as #rgb, #rrggbb, or #rrggbbaa (default white). Shows in gaps and letterbox areas." }
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
