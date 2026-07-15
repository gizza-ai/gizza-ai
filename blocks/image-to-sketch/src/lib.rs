//! gizza-ai/image-to-sketch — turn a photo into a pencil or line-art sketch.
//! Returns a grayscale PNG.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::sketch` (decode +
//! grayscale + pencil-dodge or Sobel line-art via the `image` crate) → PNG media
//! envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + image bytes output; the page
//! file-input path is ffmpeg-only — like image-pixelate-censor / add-text-to-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_to_sketch_core::{parse_mode, sketch};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    strength: f64,
}
fn default_mode() -> String {
    "pencil".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["pencil", "lineart"])
                .default("pencil")
                .describe("Sketch style: pencil (soft graphite shading, default) or lineart (clean black outlines)."),
        )
        .param(
            Param::number("strength")
                .min(0.0)
                .describe("pencil: blur radius/softness (default 8). lineart: edge sensitivity, higher = darker/thicker lines (default 1). 0 = default."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageToSketch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-to-sketch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a photo into a pencil or line-art sketch",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn a photo into a hand-drawn-style sketch. mode 'pencil' gives a soft graphite pencil-shading look (the classic color-dodge sketch effect); mode 'lineart' gives a clean black-on-white outline drawing (good for a coloring-page / contour look). strength sets the pencil softness (blur radius) or the line-art edge sensitivity; 0 = sensible default. Returns a grayscale PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ImageToSketch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-to-sketch")?;
    let mode = parse_mode(&args.mode).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let png = sketch(&bytes, mode, args.strength as f32).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "sketch.png".to_string(),
        format!("{} sketch ({} bytes PNG)", args.mode, png.len()),
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
                    "mode":     { "type": "string", "enum": ["pencil", "lineart"], "default": "pencil", "description": "Sketch style: pencil (soft graphite shading, default) or lineart (clean black outlines)." },
                    "strength": { "type": "number", "minimum": 0, "description": "pencil: blur radius/softness (default 8). lineart: edge sensitivity, higher = darker/thicker lines (default 1). 0 = default." }
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
