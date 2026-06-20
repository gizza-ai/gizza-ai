//! gizza-ai/pdf-rotate — rotate pages of a PDF by a multiple of 90 degrees.
//!
//! Loads the source PDF (URL/ref), adds `degrees` to the selected pages'
//! `/Rotate`, and returns the new PDF as a base64 envelope. `Input::Document` +
//! `degrees` (+ optional `pages`). Chat + CLI only (no page surface).
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
    degrees: i64,
    #[serde(default = "default_pages")]
    pages: String,
}
fn default_pages() -> String { "all".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::integer("degrees").required().describe("Rotation in degrees, a multiple of 90 (90, 180, 270, or -90). Added to the page's current rotation."))
        .param(Param::string("pages").default("all").describe("Which pages to rotate, 1-based, e.g. '1,3-5' or 'all' (default)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfRotate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-rotate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rotate pages of a PDF",
    requires = ["wafer-run/network"],
    skill(
        description = "Rotate pages of a PDF by a multiple of 90 degrees (90, 180, 270, or -90), added to each page's current rotation. Choose which pages with `pages` (1-based, e.g. '1,3-5' or 'all'). Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfRotate {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-rotate")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let out = gizza_ai_pdf_rotate_core::rotate(&bytes, args.degrees, &args.pages)
        .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename.strip_suffix(".pdf").map(|s| format!("{s}-rotated.pdf"))
        .unwrap_or_else(|| "rotated.pdf".to_string());

    let env = Envelope {
        for_llm: format!("rotated pages '{}' of {filename} by {} deg ({out_len}-byte PDF)", args.pages, args.degrees),
        for_ui: ForUi { data_url, mime: "application/pdf".to_string(), filename: out_name },
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
                    "url":     { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "degrees": { "type": "integer", "description": "Rotation in degrees, a multiple of 90 (90, 180, 270, or -90). Added to the page's current rotation." },
                    "pages":   { "type": "string", "default": "all", "description": "Which pages to rotate, 1-based, e.g. '1,3-5' or 'all' (default)." }
                },
                "required": ["degrees"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
