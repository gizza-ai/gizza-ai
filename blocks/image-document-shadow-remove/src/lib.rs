//! gizza-ai/image-document-shadow-remove — remove cast shadows / uneven lighting
//! from a photo of a document to produce a clean, flat, near-white page.
//! Returns a PNG.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::remove_shadow`
//! (decode + per-channel illumination estimate + division-normalise via the
//! `image` crate) → PNG media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + image bytes output; the page
//! file-input path is ffmpeg-only — like image-to-sketch / image-pixelate-censor).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_document_shadow_remove_core::{parse_mode, remove_shadow};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_whiteness")]
    whiteness: u32,
}
fn default_mode() -> String {
    "color".to_string()
}
fn default_whiteness() -> u32 {
    55
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["color", "grayscale", "blackwhite"])
                .default("color")
                .describe("Output treatment: color (keep coloured ink, default), grayscale (neutral gray), or blackwhite (crisp 1-bit scan)."),
        )
        .param(
            Param::integer("whiteness")
                .min(0.0)
                .max(100.0)
                .default(55)
                .describe("How hard to whiten the page background, 0-100 (higher snaps near-white paper to pure white; default 55)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageDocumentShadowRemove;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-document-shadow-remove",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove shadows and uneven lighting from a document photo",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Remove cast shadows and uneven lighting from a phone photo or scan of a document, flattening it to a clean near-white page while keeping the text and ink sharp. mode 'color' preserves coloured ink/highlights (default), 'grayscale' gives a neutral gray page, 'blackwhite' produces a crisp 1-bit black-and-white scan (Otsu threshold). whiteness (0-100, default 55) sets how hard the paper background is snapped to pure white. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ImageDocumentShadowRemove {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-document-shadow-remove")?;
    let mode = parse_mode(&args.mode).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let png = remove_shadow(&bytes, mode, args.whiteness).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        "document.png".to_string(),
        format!("shadow-removed {} document ({} bytes PNG)", args.mode, png.len()),
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
                    "mode":      { "type": "string", "enum": ["color", "grayscale", "blackwhite"], "default": "color", "description": "Output treatment: color (keep coloured ink, default), grayscale (neutral gray), or blackwhite (crisp 1-bit scan)." },
                    "whiteness": { "type": "integer", "minimum": 0, "maximum": 100, "default": 55, "description": "How hard to whiten the page background, 0-100 (higher snaps near-white paper to pure white; default 55)." }
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
