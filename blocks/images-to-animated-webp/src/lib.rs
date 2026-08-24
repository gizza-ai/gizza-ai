//! gizza-ai/images-to-animated-webp — combine a set of images into a single
//! animated WebP.
//!
//! Pipeline: resolve each image source (URL/ref) → pure
//! `core::animated_webp_from_images` (decode + fit-to-canvas + optional palette
//! quantization + lossless VP8L encode + hand-built animated WebP container) →
//! WebP media envelope. `Input::None` + a required `images` source_list (like
//! gif-from-images / image-collage).
//!
//! Pure Rust (no ffmpeg, no libwebp C bindings) → runs on ALL backends including
//! the chat Service Worker. Surfaces: chat + CLI. No standalone page (array
//! input + image bytes output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_images_to_animated_webp_core::{
    animated_webp_from_images, parse_color, parse_delays, Fit, Options, Order,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_delay")]
    delay_ms: u64,
    #[serde(default)]
    frame_delays_ms: String,
    #[serde(default)]
    loop_count: u64,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    max_width: u64,
    #[serde(default = "default_fit")]
    fit: String,
    #[serde(default = "default_bg")]
    background: String,
    #[serde(default)]
    colors: u64,
}
fn default_delay() -> u64 {
    200
}
fn default_order() -> String {
    "forward".to_string()
}
fn default_fit() -> String {
    "contain".to_string()
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
                .default(200)
                .describe("Delay shown for every frame, in milliseconds (10-60000, default 200 = 5 fps). Ignored for a frame that has its own value in frame_delays_ms."),
        )
        .param(
            Param::string("frame_delays_ms")
                .default("")
                .describe("Optional per-image delays in milliseconds, comma separated, one per image in the order supplied (e.g. '100,100,800' to hold the last frame). Empty = use delay_ms for every frame."),
        )
        .param(
            Param::integer("loop_count")
                .min(0.0)
                .max(65535.0)
                .default(0)
                .describe("How many times the animation plays: 0 = loop forever (default), 1 = play once, 3 = play three times."),
        )
        .param(
            Param::enumv("order", ["forward", "reverse", "boomerang"])
                .default("forward")
                .describe("Playback order of the images: 'forward' (default), 'reverse', or 'boomerang' (forward then back, ping-pong, without repeating the end frames)."),
        )
        .param(
            Param::integer("max_width")
                .min(0.0)
                .max(16383.0)
                .default(0)
                .describe("Scale the canvas down to this width in pixels, keeping the aspect ratio (0 = keep the natural size). Never scales up."),
        )
        .param(
            Param::enumv("fit", ["contain", "cover", "stretch"])
                .default("contain")
                .describe("How images that don't match the canvas aspect ratio are placed: 'contain' (default, fit inside and pad with the background), 'cover' (fill and center-crop), or 'stretch' (distort to the exact canvas size)."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Background color as #rgb, #rrggbb, #rrggbbaa, or 'transparent' (default #ffffff). Fills the padding when frames differ in size."),
        )
        .param(
            Param::integer("colors")
                .min(0.0)
                .max(256.0)
                .default(0)
                .describe("Reduce every frame to this many colors before encoding (2-256), which shrinks the file a lot for flat-color/graphic frames. 0 = keep full color (default)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImagesToAnimatedWebp;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/images-to-animated-webp",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine images into an animated WebP",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Combine a set of images into a single animated WebP (lossless VP8L), one frame per image in order — usually much smaller than the equivalent GIF and with 24-bit color plus real alpha instead of GIF's 256-color palette. delay_ms sets the per-frame delay (10-60000, default 200); frame_delays_ms overrides it per image. loop_count 0 loops forever. order can replay the frames reverse or boomerang. Frames are placed on a common canvas (the max width/height, optionally scaled down by max_width) using fit=contain|cover|stretch, padded with the background color ('transparent' supported). colors=2-256 palette-quantizes each frame for much smaller files. Provide images as a list, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ImagesToAnimatedWebp {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("images-to-animated-webp")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs(
            "images-to-animated-webp needs at least 1 image".into(),
        ));
    }

    let opts = Options {
        delay_ms: args.delay_ms.clamp(10, 60_000) as u32,
        frame_delays_ms: parse_delays(&args.frame_delays_ms).map_err(SkillError::InvalidArgs)?,
        loop_count: args.loop_count.min(65_535) as u16,
        order: Order::parse(&args.order).map_err(SkillError::InvalidArgs)?,
        max_width: args.max_width.min(16_383) as u32,
        fit: Fit::parse(&args.fit).map_err(SkillError::InvalidArgs)?,
        background: parse_color(&args.background).map_err(SkillError::InvalidArgs)?,
        colors: args.colors.min(256) as u16,
    };

    let mut imgs: Vec<Vec<u8>> = Vec::with_capacity(args.images.len());
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        imgs.push(bytes);
    }
    let count = imgs.len();

    let anim = animated_webp_from_images(&imgs, &opts).map_err(SkillError::InvalidArgs)?;
    let loops = if opts.loop_count == 0 {
        "loops forever".to_string()
    } else {
        format!("plays {}x", opts.loop_count)
    };

    build_media_envelope(
        &anim.bytes,
        "image/webp",
        "animation.webp".to_string(),
        format!(
            "animated WebP from {count} image(s): {} frame(s) at {}x{}, {} ms per loop, {loops} ({} bytes)",
            anim.frames, anim.width, anim.height, anim.duration_ms, anim.bytes.len()
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
                    "delay_ms": { "type": "integer", "minimum": 10, "maximum": 60000, "default": 200, "description": "Delay shown for every frame, in milliseconds (10-60000, default 200 = 5 fps). Ignored for a frame that has its own value in frame_delays_ms." },
                    "frame_delays_ms": { "type": "string", "default": "", "description": "Optional per-image delays in milliseconds, comma separated, one per image in the order supplied (e.g. '100,100,800' to hold the last frame). Empty = use delay_ms for every frame." },
                    "loop_count": { "type": "integer", "minimum": 0, "maximum": 65535, "default": 0, "description": "How many times the animation plays: 0 = loop forever (default), 1 = play once, 3 = play three times." },
                    "order": { "type": "string", "enum": ["forward", "reverse", "boomerang"], "default": "forward", "description": "Playback order of the images: 'forward' (default), 'reverse', or 'boomerang' (forward then back, ping-pong, without repeating the end frames)." },
                    "max_width": { "type": "integer", "minimum": 0, "maximum": 16383, "default": 0, "description": "Scale the canvas down to this width in pixels, keeping the aspect ratio (0 = keep the natural size). Never scales up." },
                    "fit": { "type": "string", "enum": ["contain", "cover", "stretch"], "default": "contain", "description": "How images that don't match the canvas aspect ratio are placed: 'contain' (default, fit inside and pad with the background), 'cover' (fill and center-crop), or 'stretch' (distort to the exact canvas size)." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Background color as #rgb, #rrggbb, #rrggbbaa, or 'transparent' (default #ffffff). Fills the padding when frames differ in size." },
                    "colors": { "type": "integer", "minimum": 0, "maximum": 256, "default": 0, "description": "Reduce every frame to this many colors before encoding (2-256), which shrinks the file a lot for flat-color/graphic frames. 0 = keep full color (default)." }
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
