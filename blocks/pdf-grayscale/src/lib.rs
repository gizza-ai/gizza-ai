//! gizza-ai/pdf-grayscale — convert a PDF to grayscale or black-and-white.
//!
//! Loads the source PDF (URL/ref), rewrites the color operators in the selected
//! pages' content streams so vector graphics and text render in gray (RGB/CMYK
//! fills and strokes → gray by luminance), optionally grayscales embedded JPEG
//! images, and returns the new PDF as a base64 envelope. `Input::Document` +
//! `mode` / `convert_images` / `threshold` / `pages`. Chat + CLI only (no page
//! surface — a standalone page can't fetch an arbitrary PDF).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// PDF byte cap (16 MiB) — generous for documents while bounding the memory a
/// single conversion pulls into the wasm sandbox.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_convert_images")]
    convert_images: bool,
    #[serde(default = "default_threshold")]
    threshold: i64,
    #[serde(default = "default_pages")]
    pages: String,
}
fn default_mode() -> String {
    "grayscale".to_string()
}
fn default_convert_images() -> bool {
    true
}
fn default_threshold() -> i64 {
    128
}
fn default_pages() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("mode", ["grayscale", "black-white"])
                .default("grayscale")
                .describe("Output mode: 'grayscale' (default) maps colors to gray by luminance; 'black-white' snaps each gray value to pure black or white at `threshold`."),
        )
        .param(
            Param::boolean("convert_images")
                .default(true)
                .describe("When true (default), also grayscale embedded JPEG images on the selected pages. Images in other codecs are left unchanged. When false, images are untouched."),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .default(128)
                .describe("Black/white cutoff, 0–255 (default 128); only used when mode is 'black-white'. Gray values at or above the cutoff become white, below become black."),
        )
        .param(
            Param::string("pages")
                .default("all")
                .describe("Which pages to convert, 1-based, e.g. '1,3-5' or 'all' (default)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfGrayscale;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-grayscale",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PDF to grayscale or black-and-white",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert a PDF's colors to grayscale, or to pure black-and-white, in place. Recolors vector graphics and text (DeviceRGB/DeviceCMYK fills and strokes become gray via perceptual luminance). With `convert_images` (default true) it also grayscales embedded JPEG images on the selected pages; images in other codecs are left unchanged. Use `mode` = 'grayscale' (default) or 'black-white' (each gray value is snapped to pure black or white at `threshold`, 0–255, default 128). Choose which pages with `pages` (1-based, e.g. '1,3-5' or 'all'). Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfGrayscale {
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
    use gizza_ai_pdf_grayscale_core::{grayscale, Mode, Options};

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-grayscale")?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let mode_label = if mode == Mode::BlackWhite {
        "black-and-white"
    } else {
        "grayscale"
    };
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let opts = Options {
        mode,
        convert_images: args.convert_images,
        threshold: args.threshold,
        pages: args.pages.clone(),
    };
    let summary = grayscale(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let out_len = summary.bytes.len();

    let encoded = B64.encode(&summary.bytes);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-{mode_label}.pdf"))
        .unwrap_or_else(|| format!("{mode_label}.pdf"));

    let mut for_llm = format!(
        "converted {} page(s) of {filename} to {mode_label}",
        summary.pages_converted
    );
    if summary.images_converted > 0 {
        for_llm.push_str(&format!(
            "; grayscaled {} embedded JPEG image(s)",
            summary.images_converted
        ));
    }
    if summary.images_skipped > 0 {
        for_llm.push_str(&format!(
            "; {} non-JPEG image(s) left unchanged",
            summary.images_skipped
        ));
    }
    for_llm.push_str(&format!(" ({out_len}-byte PDF)"));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["grayscale", "black-white"], "default": "grayscale", "description": "Output mode: 'grayscale' (default) maps colors to gray by luminance; 'black-white' snaps each gray value to pure black or white at `threshold`." },
                    "convert_images": { "type": "boolean", "default": true, "description": "When true (default), also grayscale embedded JPEG images on the selected pages. Images in other codecs are left unchanged. When false, images are untouched." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "default": 128, "description": "Black/white cutoff, 0–255 (default 128); only used when mode is 'black-white'. Gray values at or above the cutoff become white, below become black." },
                    "pages": { "type": "string", "default": "all", "description": "Which pages to convert, 1-based, e.g. '1,3-5' or 'all' (default)." }
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
    fn args_default_mode_and_flags() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/doc.pdf"}"#).unwrap();
        assert_eq!(a.mode, "grayscale");
        assert!(a.convert_images);
        assert_eq!(a.threshold, 128);
        assert_eq!(a.pages, "all");
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
