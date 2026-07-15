//! gizza-ai/sharpen-image — sharpen an image with an adjustable unsharp mask.
//! Returns a PNG. Pure-Rust (image crate) — runs on all backends incl. the chat
//! SW. Surfaces: chat + CLI (image input + image bytes output → no page, like
//! normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_sharpen_image_core::sharpen;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_amount")]
    amount: f64,
    #[serde(default)]
    threshold: i32,
}
fn default_amount() -> f64 {
    2.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("amount")
                .min(0.1)
                .max(50.0)
                .describe("Unsharp strength (Gaussian sigma; default 2.0). Higher sharpens more; 1-3 is typical."),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .describe("Minimum brightness difference a pixel needs before it is sharpened (0-255, default 0). Raise it to avoid sharpening noise in flat areas."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SharpenImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sharpen-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sharpen an image with an unsharp mask",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Sharpen an image using an adjustable unsharp mask. amount is the unsharp strength (Gaussian sigma; default 2.0, higher = sharper); threshold (0-255, default 0) is the minimum brightness difference before a pixel is sharpened, which avoids amplifying noise in flat areas. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl SharpenImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("sharpen-image")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = sharpen(&bytes, args.amount, args.threshold).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "sharpened.png".to_string(),
        format!(
            "sharpened image (amount {}, threshold {}, {} bytes PNG)",
            args.amount,
            args.threshold,
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "amount":    { "type": "number", "minimum": 0.1, "maximum": 50, "description": "Unsharp strength (Gaussian sigma; default 2.0). Higher sharpens more; 1-3 is typical." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Minimum brightness difference a pixel needs before it is sharpened (0-255, default 0). Raise it to avoid sharpening noise in flat areas." }
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
