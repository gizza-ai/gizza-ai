//! gizza-ai/blur-image — apply a Gaussian blur of adjustable radius to an image.
//! Returns a PNG. Pure-Rust (image crate) — runs on all backends incl. the chat
//! SW. Surfaces: chat + CLI (image input + image bytes output → no page, like
//! sharpen-image / normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_blur_image_core::blur;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_radius")]
    radius: f64,
}
fn default_radius() -> f64 {
    5.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::number("radius")
            .min(0.1)
            .max(200.0)
            .describe("Gaussian blur radius in pixels (standard deviation; default 5.0). Higher blurs more; 2-10 is typical, 20+ is a heavy soft wash."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BlurImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/blur-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a Gaussian blur of adjustable radius to an image.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Apply a Gaussian blur of adjustable radius to an entire image. radius is the blur strength in pixels (Gaussian standard deviation; default 5.0, higher = blurrier; 2-10 is typical). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl BlurImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("blur-image")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = blur(&bytes, args.radius).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "blurred.png".to_string(),
        format!(
            "blurred image (radius {}, {} bytes PNG)",
            args.radius,
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "radius": { "type": "number", "minimum": 0.1, "maximum": 200, "description": "Gaussian blur radius in pixels (standard deviation; default 5.0). Higher blurs more; 2-10 is typical, 20+ is a heavy soft wash." }
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
