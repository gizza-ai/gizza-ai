//! gizza-ai/image-info — report an image's format, dimensions, color type, and
//! file size from its bytes.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::info` (decode + inspect)
//! → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report; the page file-input
//! path is ffmpeg-only — the F3 no-page file-input pattern, like detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    format: String,
    mime: String,
    width: u32,
    height: u32,
    megapixels: f64,
    aspect_ratio: String,
    color_type: String,
    channels: u8,
    bits_per_pixel: u16,
    has_alpha: bool,
    /// File size in bytes.
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageInfo;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report an image's format, dimensions, and color type",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Report an image's properties without uploading it: format (PNG/JPEG/WebP/GIF/BMP/TIFF/ICO), pixel dimensions, megapixels, aspect ratio, color type (RGB/RGBA/grayscale and bit depth), channel count, bits per pixel, whether it has an alpha channel, and the file size in bytes. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageInfo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-info")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let i = gizza_ai_image_info_core::info(&bytes).map_err(SkillError::InvalidArgs)?;
    let megapixels = ((i.width as f64 * i.height as f64) / 1_000_000.0 * 100.0).round() / 100.0;

    let resp = Resp {
        format: i.format,
        mime: i.mime,
        width: i.width,
        height: i.height,
        megapixels,
        aspect_ratio: i.aspect_ratio,
        color_type: i.color_type,
        channels: i.channels,
        bits_per_pixel: i.bits_per_pixel,
        has_alpha: i.has_alpha,
        bytes: bytes.len(),
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-info response: {e}")))
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
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
