//! gizza-ai/colorblind-simulator — simulate colour-vision deficiency on an image.
//! Returns a PNG. Pure-Rust (image crate). Surfaces: chat + CLI (image input +
//! image bytes output → no page, like normalize-image / sharpen-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_colorblind_simulator_core::{simulate, Kind};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_type")]
    r#type: String,
}
fn default_type() -> String {
    "deuteranopia".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::enumv("type", ["protanopia", "deuteranopia", "tritanopia"])
            .default("deuteranopia")
            .describe("Colour-vision deficiency to simulate: protanopia (red), deuteranopia (green, default), or tritanopia (blue)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorblindSimulator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/colorblind-simulator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Simulate colour-blindness on an image",
    requires = ["wafer-run/network"],
    skill(
        description = "Simulate how an image looks to someone with a colour-vision deficiency, to check a design's accessibility. type = protanopia (red-deficient), deuteranopia (green-deficient, default), or tritanopia (blue-deficient); the standard CVD simulation matrix is applied to every pixel (alpha preserved). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ColorblindSimulator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("colorblind-simulator")?;
    let kind = Kind::parse(&args.r#type).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = simulate(&bytes, kind).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        format!("colorblind-{}.png", args.r#type),
        format!("simulated {} ({} bytes PNG)", args.r#type, png.len()),
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
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "type": { "type": "string", "enum": ["protanopia", "deuteranopia", "tritanopia"], "default": "deuteranopia", "description": "Colour-vision deficiency to simulate: protanopia (red), deuteranopia (green, default), or tritanopia (blue)." }
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
