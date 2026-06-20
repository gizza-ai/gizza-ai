//! gizza-ai/images-to-pdf — combine several images into one PDF (one per page).
//!
//! Pipeline: load each image source via `block-utils` `resolve_source` (URL fetch
//! or attachment ref) → pure `core::images_to_pdf` (decode + embed + paginate) →
//! base64 PDF envelope. `Input::None` + a required `images` source_list (like
//! merge-pdf — the sources arrive as an array param, not a single media input).
//! Surfaces: chat + CLI. No standalone page (array input + pure-Rust PDF bytes).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::source_list("images", 1)
            .required()
            .describe("Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`. One image per PDF page, in order."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImagesToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/images-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine images into a single PDF (one image per page)",
    requires = ["wafer-run/network"],
    skill(
        description = "Combine a set of images into a single PDF, one image per page, in the given order. Each image is a URL or a `ref` to an uploaded image attachment (PNG/JPEG/WebP/GIF/BMP). Provide at least one image.",
        parameters = schema_json()
    ),
)]
impl ImagesToPdf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("images-to-pdf")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs("images-to-pdf needs at least 1 image".into()));
    }
    let mut imgs: Vec<Vec<u8>> = Vec::with_capacity(args.images.len());
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        imgs.push(bytes);
    }

    let pdf = gizza_ai_images_to_pdf_core::images_to_pdf(&imgs).map_err(SkillError::InvalidArgs)?;
    if pdf.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output PDF is {} bytes, over the {MAX_OUTPUT_BYTES} cap",
            pdf.len()
        )));
    }
    let out_len = pdf.len();
    let encoded = B64.encode(&pdf);
    let data_url = format!("data:application/pdf;base64,{encoded}");

    let env = Envelope {
        for_llm: format!("combined {} images into a {out_len}-byte PDF (images.pdf)", imgs.len()),
        for_ui: ForUi { data_url, mime: "application/pdf".to_string(), filename: "images.pdf".to_string() },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (a required `images` source_list, minItems 1).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "images": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Ordered list of image sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`. One image per PDF page, in order.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["images"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_image_list() {
        let a: Args = serde_json::from_str(r#"{"images":[{"url":"https://x/a.png"},{"ref":"c_1"}]}"#).unwrap();
        assert_eq!(a.images.len(), 2);
    }
}
