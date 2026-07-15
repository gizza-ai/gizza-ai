//! gizza-ai/normalize-image — auto-normalize (contrast-stretch) an image to the
//! full dynamic range. Returns a PNG. Pure-Rust (image crate) — runs on all
//! backends incl. the chat SW. Surfaces: chat + CLI (image input + image bytes
//! output → no page, like image-pixelate-censor).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_normalize_image_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    clip_percent: f64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::number("clip_percent")
            .min(0.0)
            .max(45.0)
            .describe("Percent of the darkest/lightest pixels per channel to ignore before stretching (0-45, default 0 = pure min/max auto-level). A small value like 1-2 ignores outliers."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NormalizeImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/normalize-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-normalize an image (histogram stretch)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Auto-normalize (contrast-stretch) an image by mapping each color channel's used range to the full 0-255 dynamic range, brightening flat/low-contrast photos. clip_percent (0-45, default 0) ignores that fraction of extreme pixels per channel first so outliers don't flatten the result. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl NormalizeImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("normalize-image")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = normalize(&bytes, args.clip_percent).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "normalized.png".to_string(),
        format!("normalized image (clip {}%, {} bytes PNG)", args.clip_percent, png.len()),
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
                    "url":          { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "clip_percent": { "type": "number", "minimum": 0, "maximum": 45, "description": "Percent of the darkest/lightest pixels per channel to ignore before stretching (0-45, default 0 = pure min/max auto-level). A small value like 1-2 ignores outliers." }
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
