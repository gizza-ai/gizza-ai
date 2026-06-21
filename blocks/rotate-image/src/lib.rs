//! gizza-ai/rotate-image — rotate an image clockwise by 90/180/270 degrees
//! (lossless) or any arbitrary angle (bilinear resample into an enlarged canvas
//! with a configurable background fill). Returns a PNG. Pure-Rust (image crate) —
//! runs on all backends incl. the chat SW. Surfaces: chat + CLI (image input +
//! image bytes output → no page, like flip-image / normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_rotate_image_core::{parse_color, rotate};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_angle")]
    angle: f64,
    #[serde(default = "default_background")]
    background: String,
}

fn default_angle() -> f64 {
    90.0
}

fn default_background() -> String {
    "transparent".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("angle")
                .default(90.0)
                .min(-360.0)
                .max(360.0)
                .describe(
                    "Rotation angle in degrees, clockwise. 90/180/270 are lossless; any other value (e.g. 45, -30) resamples and enlarges the canvas to fit, filling exposed corners with the background.",
                ),
        )
        .param(
            Param::string("background")
                .default("transparent")
                .describe(
                    "Fill color for corners exposed by an arbitrary-angle rotation: transparent (default), white, black, or a hex color like #rrggbb / #rrggbbaa. Ignored for 90/180/270.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RotateImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rotate-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rotate an image by 90/180/270 or an arbitrary angle",
    requires = ["wafer-run/network"],
    skill(
        description = "Rotate an image clockwise by 90, 180, or 270 degrees (lossless) or by any arbitrary angle (e.g. 45, -30; the canvas is enlarged to fit and exposed corners are filled with the background color). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl RotateImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("rotate-image")?;
    let bg = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = rotate(&bytes, args.angle, bg).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "rotated.png".to_string(),
        format!("rotated image ({}°, {} bytes PNG)", args.angle, png.len()),
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
                    "angle":      { "type": "number", "default": 90.0, "minimum": -360, "maximum": 360, "description": "Rotation angle in degrees, clockwise. 90/180/270 are lossless; any other value (e.g. 45, -30) resamples and enlarges the canvas to fit, filling exposed corners with the background." },
                    "background": { "type": "string", "default": "transparent", "description": "Fill color for corners exposed by an arbitrary-angle rotation: transparent (default), white, black, or a hex color like #rrggbb / #rrggbbaa. Ignored for 90/180/270." }
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
