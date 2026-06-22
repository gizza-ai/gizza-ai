//! gizza-ai/image-hsl-adjust — shift hue and scale saturation and lightness of an
//! image in HSL space. Returns a PNG. Pure Rust (image crate) — runs on all
//! backends incl. the chat SW. Surfaces: chat + CLI (image input + image bytes
//! output → no page, like normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_hsl_adjust_core::adjust;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    hue: f64,
    #[serde(default = "one")]
    saturation: f64,
    #[serde(default = "one")]
    lightness: f64,
}
fn one() -> f64 {
    1.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("hue")
                .min(-360.0)
                .max(360.0)
                .describe("Hue shift in degrees, -360 to 360 (0 = no change)."),
        )
        .param(
            Param::number("saturation")
                .min(0.0)
                .max(4.0)
                .default(1.0)
                .describe("Saturation scale factor (0 = grayscale, 1 = unchanged, >1 = more vivid; max 4)."),
        )
        .param(
            Param::number("lightness")
                .min(0.0)
                .max(4.0)
                .default(1.0)
                .describe("Lightness scale factor (0 = black, 1 = unchanged, >1 = brighter; max 4)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageHslAdjust;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-hsl-adjust",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Adjust image hue/saturation/lightness (HSL)",
    requires = ["wafer-run/network"],
    skill(
        description = "Shift hue and scale saturation and lightness of an image in HSL space. hue is a degree shift (-360..360, 0 = no change); saturation and lightness are scale factors (0 = none/black, 1 = unchanged, >1 = more). E.g. saturation=0 makes it grayscale, hue=180 rotates colors to their complement. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageHslAdjust {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-hsl-adjust")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = adjust(&bytes, args.hue as f32, args.saturation as f32, args.lightness as f32)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "hsl-adjusted.png".to_string(),
        format!(
            "HSL adjusted (hue {:+}°, sat ×{}, light ×{}) — {} bytes PNG",
            args.hue, args.saturation, args.lightness, png.len()
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
                    "hue":        { "type": "number", "minimum": -360, "maximum": 360, "description": "Hue shift in degrees, -360 to 360 (0 = no change)." },
                    "saturation": { "type": "number", "minimum": 0, "maximum": 4, "default": 1.0, "description": "Saturation scale factor (0 = grayscale, 1 = unchanged, >1 = more vivid; max 4)." },
                    "lightness":  { "type": "number", "minimum": 0, "maximum": 4, "default": 1.0, "description": "Lightness scale factor (0 = black, 1 = unchanged, >1 = brighter; max 4)." }
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
