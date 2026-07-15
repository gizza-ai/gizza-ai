//! gizza-ai/extract-pdf-images — fetch a PDF (URL or attachment ref), extract its
//! embedded raster images, and return them bundled in a ZIP.
//!
//! Pipeline: resolve the source document → `core::extract_images_zip` (pure-Rust
//! lopdf + image + zip) → base64 envelope (the ZIP as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the F3 "no-page file-input"
//! pattern, like encrypt-file).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB (PDFs with photos get large)

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::Document` emits the `url`⊕`ref` `oneOf` — a PDF arrives via URL fetch
/// or an attachment ref. No other parameters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractPdfImages;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-pdf-images",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract embedded images from a PDF as a ZIP",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the embedded raster images from a PDF and return them bundled in a ZIP. JPEG (DCTDecode) and JPEG-2000 images are saved losslessly as-is; 8-bit grayscale/RGB images are saved as PNG. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call). This extracts stored image objects — it does not rasterize/screenshot pages.",
        parameters = schema_json()
    ),
)]
impl ExtractPdfImages {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("extract-pdf-images")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let (zip, summary) = gizza_ai_extract_pdf_images_core::extract_images_zip(&bytes)
        .map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "zip");
    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");

    let skipped_note = if summary.skipped > 0 {
        format!(
            " ({} image(s) skipped — unsupported filter/colour space)",
            summary.skipped
        )
    } else {
        String::new()
    };
    let for_llm = format!(
        "extracted {} image(s) from {in_filename} into {filename} ({zip_len}-byte ZIP){skipped_note}",
        summary.extracted
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/zip".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Document url⊕ref oneOf, no other params).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
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

    #[test]
    fn output_filename_is_zip() {
        assert_eq!(replace_extension("report.pdf", "zip"), "report.zip");
    }
}
