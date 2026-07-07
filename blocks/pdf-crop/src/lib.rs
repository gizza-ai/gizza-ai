//! gizza-ai/pdf-crop — crop the margins of a PDF's pages.
//!
//! Loads the source PDF (URL/ref) and writes a new `/CropBox` on the selected
//! pages, inset from each page's current visible box by the given per-side
//! margins (`top`/`bottom`/`left`/`right` in `pt`/`mm`/`in`). Returns the new
//! PDF as a base64 envelope. `Input::Document` + margins + `unit` + `pages`.
//! Chat + CLI only (PDF bytes have no page surface).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    top: f64,
    #[serde(default)]
    bottom: f64,
    #[serde(default)]
    left: f64,
    #[serde(default)]
    right: f64,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default = "default_pages")]
    pages: String,
}
fn default_unit() -> String {
    "pt".to_string()
}
fn default_pages() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::number("top").default(0.0).min(0.0).describe(
            "Amount to crop off the TOP edge, in `unit` (default 0 = no crop). E.g. 0.5 with unit=in.",
        ))
        .param(Param::number("bottom").default(0.0).min(0.0).describe(
            "Amount to crop off the BOTTOM edge, in `unit` (default 0 = no crop).",
        ))
        .param(Param::number("left").default(0.0).min(0.0).describe(
            "Amount to crop off the LEFT edge, in `unit` (default 0 = no crop).",
        ))
        .param(Param::number("right").default(0.0).min(0.0).describe(
            "Amount to crop off the RIGHT edge, in `unit` (default 0 = no crop).",
        ))
        .param(
            Param::enumv("unit", ["pt", "mm", "in"])
                .default("pt")
                .describe("Unit for the margins: pt (points, 1/72\"), mm, or in (default pt)."),
        )
        .param(Param::string("pages").default("all").describe(
            "Which pages to crop, 1-based: 'all' (default), 'odd', 'even', or a list/range like '1,3-5'.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfCrop;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-crop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crop the margins of a PDF",
    requires = ["wafer-run/network"],
    skill(
        description = "Crop the margins of a PDF by trimming each page's edges. Give per-side amounts top/bottom/left/right in `unit` (pt, mm, or in) — set them equal for a uniform crop, or individually. Choose which pages with `pages` (1-based: 'all', 'odd', 'even', or '1,3-5'). Non-destructive: it writes each page's /CropBox and leaves the content untouched. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfCrop {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-crop")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let out = gizza_ai_pdf_crop_core::crop(
        &bytes,
        args.top,
        args.bottom,
        args.left,
        args.right,
        &args.unit,
        &args.pages,
    )
    .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-cropped.pdf"))
        .unwrap_or_else(|| "cropped.pdf".to_string());

    let env = Envelope {
        for_llm: format!(
            "cropped pages '{}' of {filename}: top {} bottom {} left {} right {} {} ({out_len}-byte PDF)",
            args.pages, args.top, args.bottom, args.left, args.right, args.unit
        ),
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "top":    { "type": "number", "minimum": 0, "default": 0.0, "description": "Amount to crop off the TOP edge, in `unit` (default 0 = no crop). E.g. 0.5 with unit=in." },
                    "bottom": { "type": "number", "minimum": 0, "default": 0.0, "description": "Amount to crop off the BOTTOM edge, in `unit` (default 0 = no crop)." },
                    "left":   { "type": "number", "minimum": 0, "default": 0.0, "description": "Amount to crop off the LEFT edge, in `unit` (default 0 = no crop)." },
                    "right":  { "type": "number", "minimum": 0, "default": 0.0, "description": "Amount to crop off the RIGHT edge, in `unit` (default 0 = no crop)." },
                    "unit":   { "type": "string", "enum": ["pt", "mm", "in"], "default": "pt", "description": "Unit for the margins: pt (points, 1/72\"), mm, or in (default pt)." },
                    "pages":  { "type": "string", "default": "all", "description": "Which pages to crop, 1-based: 'all' (default), 'odd', 'even', or a list/range like '1,3-5'." }
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
