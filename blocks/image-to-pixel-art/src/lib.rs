//! gizza-ai/image-to-pixel-art — turn a photo into limited-palette pixel art by
//! downscaling to a coarse grid, reducing to an image-derived palette (NeuQuant),
//! and upscaling back with nearest-neighbour for crisp blocks. Returns a PNG.
//! Pure Rust (image + color_quant) — runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_to_pixel_art_core::pixelate;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_pixel_size")]
    pixel_size: u64,
    #[serde(default = "default_colors")]
    colors: u64,
}
fn default_pixel_size() -> u64 {
    8
}
fn default_colors() -> u64 {
    16
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("pixel_size")
                .min(2.0)
                .max(64.0)
                .default(8)
                .describe("Size of each pixel-art block in source pixels, 2-64 (default 8). Larger = chunkier, more retro."),
        )
        .param(
            Param::integer("colors")
                .min(2.0)
                .max(256.0)
                .default(16)
                .describe("Number of colors in the palette, 2-256 (default 16). The palette is derived from the image."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageToPixelArt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-to-pixel-art",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn an image into limited-palette pixel art",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn a photo into retro pixel art: downscale to a coarse grid of chunky blocks and reduce to a limited palette derived from the image (NeuQuant). pixel_size is the block size 2-64 (default 8; larger = chunkier); colors is the palette size 2-256 (default 16). Returns a PNG at roughly the original dimensions. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageToPixelArt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-to-pixel-art")?;
    let pixel_size = args.pixel_size.clamp(2, 64) as u32;
    let colors = args.colors.clamp(2, 256) as usize;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = pixelate(&bytes, pixel_size, colors).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "pixel-art.png".to_string(),
        format!(
            "pixel art ({pixel_size}px blocks, {colors} colors, {} bytes PNG)",
            png.len()
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
                    "url":        { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "pixel_size": { "type": "integer", "minimum": 2, "maximum": 64, "default": 8, "description": "Size of each pixel-art block in source pixels, 2-64 (default 8). Larger = chunkier, more retro." },
                    "colors":     { "type": "integer", "minimum": 2, "maximum": 256, "default": 16, "description": "Number of colors in the palette, 2-256 (default 16). The palette is derived from the image." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
