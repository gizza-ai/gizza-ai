//! gizza-ai/gif-from-images — combine a set of images into a single animated GIF.
//!
//! Pipeline: resolve each image source (URL/ref) → pure `core::gif_from_bytes`
//! (decode + fit-to-canvas + GIF encode via the `image` crate) → GIF media
//! envelope. `Input::None` + a required `images` source_list (like image-collage).
//!
//! Pure Rust (no ffmpeg) → runs on ALL backends including the chat Service
//! Worker. Surfaces: chat + CLI. No standalone page (array input + image bytes
//! output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_gif_from_images_core::{gif_from_bytes, parse_color};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_delay")]
    delay_ms: u64,
    #[serde(default = "default_bg")]
    background: String,
}
fn default_delay() -> u64 {
    500
}
fn default_bg() -> String {
    "#ffffff".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 1)
                .required()
                .describe("Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP) to animate, one per frame. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::integer("delay_ms")
                .min(10.0)
                .max(60000.0)
                .default(500)
                .describe("Delay per frame in milliseconds (10-60000, default 500)."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background color as #rgb, #rrggbb, or #rrggbbaa (default white). Fills the letterbox padding when frames differ in size."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GifFromImages;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-from-images",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine images into an animated GIF",
    requires = ["wafer-run/network"],
    skill(
        description = "Combine a set of images into a single animated GIF, one frame per image in order. delay_ms sets the per-frame delay (10-60000, default 500). Frames are scaled to fit a common canvas (the max width/height) with aspect ratio preserved, padded with the background color. Provide images as a list, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP). Loops forever.",
        parameters = schema_json()
    ),
)]
impl GifFromImages {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("gif-from-images")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs("gif-from-images needs at least 1 image".into()));
    }
    let bg = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;
    let delay = args.delay_ms.clamp(10, 60_000) as u16;

    let mut imgs: Vec<Vec<u8>> = Vec::with_capacity(args.images.len());
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        imgs.push(bytes);
    }
    let count = imgs.len();

    let gif = gif_from_bytes(&imgs, delay, bg).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &gif,
        "image/gif",
        "animation.gif".to_string(),
        format!("animated GIF from {count} image(s), {delay}ms/frame ({} bytes)", gif.len()),
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
                        "description": "Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP) to animate, one per frame. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "delay_ms":   { "type": "integer", "minimum": 10, "maximum": 60000, "default": 500, "description": "Delay per frame in milliseconds (10-60000, default 500)." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background color as #rgb, #rrggbb, or #rrggbbaa (default white). Fills the letterbox padding when frames differ in size." }
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
