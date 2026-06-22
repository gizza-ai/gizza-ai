//! gizza-ai/image-round-avatar — crop an image into a circle or rounded-square
//! avatar with transparent corners. Returns a PNG.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::avatar` (center-crop +
//! resize + anti-aliased alpha mask via the `image` crate) → PNG media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + image bytes output; the page
//! file-input path is ffmpeg-only — like add-text-to-image / image-pixelate-censor).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_round_avatar_core::{avatar, parse_shape};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_shape")]
    shape: String,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    radius: Option<u32>,
}
fn default_shape() -> String {
    "circle".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::enumv("shape", ["circle", "rounded"]).default("circle").describe("Avatar shape: circle (default) or rounded (rounded square)."))
        .param(Param::integer("size").min(1.0).describe("Output square size in pixels (default: the cropped square's side). The image is center-cropped to a square first."))
        .param(Param::integer("radius").min(0.0).describe("Corner radius in pixels for the rounded shape (default ~20% of the side). Ignored for circle."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageRoundAvatar;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-round-avatar",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crop an image into a circle or rounded-square avatar",
    requires = ["wafer-run/network"],
    skill(
        description = "Crop an image into a circular or rounded-square avatar with transparent corners, returned as a PNG. The image is center-cropped to a square first. shape is 'circle' (default) or 'rounded'; size sets the output square size in pixels; radius sets the corner radius for the rounded shape. Edges are anti-aliased. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageRoundAvatar {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-round-avatar")?;
    let shape = parse_shape(&args.shape).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let png = avatar(&bytes, shape, args.size, args.radius).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "avatar.png".to_string(),
        format!("cropped image into a {} avatar ({} bytes PNG)", args.shape, png.len()),
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "shape":  { "type": "string", "enum": ["circle", "rounded"], "default": "circle", "description": "Avatar shape: circle (default) or rounded (rounded square)." },
                    "size":   { "type": "integer", "minimum": 1, "description": "Output square size in pixels (default: the cropped square's side). The image is center-cropped to a square first." },
                    "radius": { "type": "integer", "minimum": 0, "description": "Corner radius in pixels for the rounded shape (default ~20% of the side). Ignored for circle." }
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
