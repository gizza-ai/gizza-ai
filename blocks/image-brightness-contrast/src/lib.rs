//! gizza-ai/image-brightness-contrast — adjust the brightness and contrast of an
//! image by signed amounts. Returns a PNG. Pure Rust (image crate) — runs on all
//! backends incl. the chat SW. Surfaces: chat + CLI (this handler) and the
//! standalone page (web/, `wasm-image` runtime) — both call core `apply` with
//! the same `Params`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_brightness_contrast_core::{apply, Params};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(flatten)]
    params: Params,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("brightness")
                .min(-255.0)
                .max(255.0)
                .describe("Brightness adjustment, -255 to 255 (negative darkens, positive brightens; 0 = no change)."),
        )
        .param(
            Param::number("contrast")
                .min(-100.0)
                .max(100.0)
                .describe("Contrast adjustment, -100 to 100 (negative flattens, positive increases; 0 = no change)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageBrightnessContrast;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-brightness-contrast",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Adjust image brightness and contrast",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Adjust the brightness and contrast of an image by signed amounts. brightness is -255..255 (negative darkens, positive brightens); contrast is -100..100 (negative flattens, positive increases); 0/0 leaves the image unchanged. Contrast is applied around mid-gray, then brightness. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageBrightnessContrast {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-brightness-contrast")?;
    let (brightness, contrast) = args.params.normalized();
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let (png, out_name) = apply(&bytes, &args.params).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        out_name,
        format!("adjusted brightness {brightness}, contrast {contrast} ({} bytes PNG)", png.len()),
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
                    "brightness": { "type": "integer", "minimum": -255, "maximum": 255, "description": "Brightness adjustment, -255 to 255 (negative darkens, positive brightens; 0 = no change)." },
                    "contrast":   { "type": "number", "minimum": -100, "maximum": 100, "description": "Contrast adjustment, -100 to 100 (negative flattens, positive increases; 0 = no change)." }
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
