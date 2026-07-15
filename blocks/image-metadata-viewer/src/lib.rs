//! gizza-ai/image-metadata-viewer — read EXIF/TIFF metadata (camera, GPS,
//! timestamps) from an image.
//!
//! Pipeline: resolve the source image → `core::read_metadata` (pure, kamadak-exif)
//! → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image file input + text output, the no-page
//! file-input pattern, like detect-file-type / image-info).
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

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageMetadataViewer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-metadata-viewer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Read EXIF/GPS metadata from an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read and display EXIF/TIFF metadata embedded in an image — camera make/model, exposure/aperture/ISO, lens, orientation, timestamps, and GPS. Returns every metadata field (tag, value, IFD) plus the GPS location decoded to decimal latitude/longitude when present. Works on JPEG/TIFF/HEIF/PNG/WebP that carry EXIF. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageMetadataViewer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-metadata-viewer")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let meta = gizza_ai_image_metadata_viewer_core::read_metadata(&bytes)
        .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&meta)
        .map_err(|e| SkillError::Serialize(format!("serialize image-metadata-viewer response: {e}")))
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
