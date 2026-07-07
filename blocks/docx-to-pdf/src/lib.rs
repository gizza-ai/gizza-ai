//! gizza-ai/docx-to-pdf — convert a Word `.docx` document into a paginated PDF.
//!
//! Pipeline: resolve the source `.docx` (URL fetch or attachment ref) →
//! `core::docx_to_pdf` (zip + WordprocessingML parse + lopdf PDF assembly) →
//! base64 `application/pdf` envelope (`build_media_envelope`) the chat UI offers
//! as a download.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page: a binary file input with binary (PDF) output
//! fits neither the pure-text page nor the ffmpeg file→media page shape — the
//! no-page file-input pattern (like pdf-to-epub / markdown-to-pdf).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_docx_to_pdf_core::docx_to_pdf;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024; // 32 MiB
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_margin")]
    margin: f64,
    #[serde(default = "default_page_size")]
    page_size: String,
    #[serde(default)]
    page_numbers: bool,
}
fn default_font_size() -> f64 {
    11.0
}
fn default_margin() -> f64 {
    72.0
}
fn default_page_size() -> String {
    "letter".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::number("font_size")
                .min(6.0)
                .max(48.0)
                .describe("Base body font size in points for text without its own size (default 11). Text that carries an explicit size in the document keeps it; headings are scaled up from this."),
        )
        .param(
            Param::number("margin")
                .min(0.0)
                .max(300.0)
                .describe("Page margin in points (default 72 = 1 inch). 72 points = 1 inch."),
        )
        .param(
            Param::enumv("page_size", ["letter", "a4"])
                .default("letter")
                .describe("Page size: 'letter' (US Letter, 8.5x11in, default) or 'a4' (210x297mm)."),
        )
        .param(
            Param::boolean("page_numbers")
                .default(false)
                .describe("When true, draw a centered 'n / total' page number in the bottom margin of each page. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DocxToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docx-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Word .docx document into a paginated PDF",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert a Word .docx document into a clean, paginated PDF (US Letter by default, or A4). Carries over paragraphs, heading and title styles (scaled and bold), bold/italic text, explicit run font sizes, paragraph alignment (left/center/right; justify renders left-aligned), bullet and numbered list items (rendered with bullet markers, indented by level), hard line breaks, explicit page breaks, and tables (flattened to readable pipe-separated rows). Provide the .docx as url (HTTP/HTTPS) or ref from a prior tool call. font_size (base body size in points, default 11), margin (points, default 72 = 1 inch), page_size (letter or a4), and optional page_numbers are configurable. It is a lightweight structural converter using the built-in Helvetica fonts — it does not embed images or reproduce exact Word page layout. Returns a PDF. Runs locally — the document never leaves the device.",
        parameters = schema_json()
    ),
)]
impl DocxToPdf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("docx-to-pdf")?;

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    let pdf = docx_to_pdf(&bytes, args.font_size, args.margin, &args.page_size, args.page_numbers)
        .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &pdf,
        "application/pdf",
        "document.pdf".to_string(),
        format!("converted a Word .docx into a PDF ({} bytes)", pdf.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema. `Input::Document` emits the `url`⊕`ref` oneOf; the four scalar
    /// params follow.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "font_size": { "type": "number", "minimum": 6, "maximum": 48, "description": "Base body font size in points for text without its own size (default 11). Text that carries an explicit size in the document keeps it; headings are scaled up from this." },
                    "margin":    { "type": "number", "minimum": 0, "maximum": 300, "description": "Page margin in points (default 72 = 1 inch). 72 points = 1 inch." },
                    "page_size": { "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "Page size: 'letter' (US Letter, 8.5x11in, default) or 'a4' (210x297mm)." },
                    "page_numbers": { "type": "boolean", "default": false, "description": "When true, draw a centered 'n / total' page number in the bottom margin of each page. Default false." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_with_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.docx"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.docx"));
        assert_eq!(a.font_size, 11.0);
        assert_eq!(a.margin, 72.0);
        assert_eq!(a.page_size, "letter");
        assert!(!a.page_numbers);
    }

    #[test]
    fn args_parse_ref_with_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_7","font_size":13,"margin":36,"page_size":"a4","page_numbers":true}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert_eq!(a.font_size, 13.0);
        assert_eq!(a.margin, 36.0);
        assert_eq!(a.page_size, "a4");
        assert!(a.page_numbers);
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
