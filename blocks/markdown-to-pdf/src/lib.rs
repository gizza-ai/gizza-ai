//! gizza-ai/markdown-to-pdf — convert a Markdown document into a formatted PDF.
//!
//! Pure-Rust (pulldown-cmark + lopdf, built-in Helvetica/Courier fonts), so it
//! runs on ALL backends incl. the chat Service Worker. The PDF is wrapped as an
//! `application/pdf` data-URL envelope. Surfaces: chat + CLI (text input + PDF
//! bytes output → no page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_markdown_to_pdf_core::markdown_to_pdf;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    markdown: String,
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
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown document to convert into a PDF."),
        )
        .param(
            Param::number("font_size")
                .min(6.0)
                .max(48.0)
                .describe("Base body font size in points (default 11). Headings are scaled up from this."),
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
struct MarkdownToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Markdown document into a formatted PDF",
    skill(
        description = "Convert a Markdown document into a clean, formatted, paginated PDF (US Letter). Renders headings, bold/italic, ordered and unordered (nested) lists, fenced and inline code (monospace), block quotes, tables, horizontal rules, and task lists. Content flows across as many pages as needed using the built-in Helvetica and Courier fonts. font_size (base body size in points, default 11), margin (points, default 72 = 1 inch), page_size (letter or a4), and optional page_numbers are configurable. Returns a PDF. Runs locally — the document never leaves the device.",
        parameters = schema_json()
    ),
)]
impl MarkdownToPdf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("markdown-to-pdf")?;
    let pdf = markdown_to_pdf(
        &args.markdown,
        args.font_size,
        args.margin,
        &args.page_size,
        args.page_numbers,
    )
    .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &pdf,
        "application/pdf",
        "document.pdf".to_string(),
        format!("converted Markdown to a PDF ({} bytes)", pdf.len()),
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
                    "markdown":  { "type": "string", "description": "The Markdown document to convert into a PDF." },
                    "font_size": { "type": "number", "minimum": 6, "maximum": 48, "description": "Base body font size in points (default 11). Headings are scaled up from this." },
                    "margin":    { "type": "number", "minimum": 0, "maximum": 300, "description": "Page margin in points (default 72 = 1 inch). 72 points = 1 inch." },
                    "page_size": { "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "Page size: 'letter' (US Letter, 8.5x11in, default) or 'a4' (210x297mm)." },
                    "page_numbers": { "type": "boolean", "default": false, "description": "When true, draw a centered 'n / total' page number in the bottom margin of each page. Default false." }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
