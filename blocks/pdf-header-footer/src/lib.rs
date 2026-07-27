//! gizza-ai/pdf-header-footer — stamp header/footer text onto an existing PDF.
//!
//! Loads the source PDF (URL/ref), overlays a header at the top margin and/or a
//! footer at the bottom margin on each selected page, and returns the new PDF as
//! a base64 envelope. `Input::Document` + the stamp options (header/footer text
//! and independent alignment, page range, font/size/colour/margin/opacity).
//! `{page}` and `{pages}` tokens resolve per page. Chat + CLI only (the PDF
//! family has no standalone page surface — binary-in/PDF-out has no page render
//! mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_header_footer_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    header: String,
    #[serde(default)]
    footer: String,
    #[serde(default = "d_align")]
    header_align: String,
    #[serde(default = "d_align")]
    footer_align: String,
    #[serde(default = "d_font")]
    font: String,
    #[serde(default = "d_font_size")]
    font_size: f64,
    #[serde(default = "d_color")]
    color: String,
    #[serde(default = "d_margin")]
    margin: f64,
    #[serde(default = "d_opacity")]
    opacity: f64,
    #[serde(default = "d_pages")]
    pages: String,
}

fn d_align() -> String {
    "center".to_string()
}
fn d_font() -> String {
    "helvetica".to_string()
}
fn d_font_size() -> f64 {
    10.0
}
fn d_color() -> String {
    "#444444".to_string()
}
fn d_margin() -> f64 {
    36.0
}
fn d_opacity() -> f64 {
    1.0
}
fn d_pages() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("header").default("").describe(
            "Text for the header band at the top of each page. Use {page} for the current page number and {pages} for the total page count, e.g. 'Confidential' or 'Page {page} of {pages}'. Leave empty for no header. Provide a header, a footer, or both.",
        ))
        .param(Param::string("footer").default("").describe(
            "Text for the footer band at the bottom of each page. Use {page} for the current page number and {pages} for the total page count. Leave empty for no footer. Provide a header, a footer, or both.",
        ))
        .param(
            Param::enumv("header_align", ["left", "center", "right"])
                .default("center")
                .describe("Horizontal alignment of the header text: left, center or right. Default center."),
        )
        .param(
            Param::enumv("footer_align", ["left", "center", "right"])
                .default("center")
                .describe("Horizontal alignment of the footer text: left, center or right. Default center."),
        )
        .param(
            Param::enumv("font", ["helvetica", "times", "courier"])
                .default("helvetica")
                .describe("Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica."),
        )
        .param(
            Param::number("font_size")
                .default(10.0)
                .min(4.0)
                .max(144.0)
                .describe("Font size in points (4–144). Default 10."),
        )
        .param(Param::string("color").default("#444444").describe(
            "Text colour as a hex code, e.g. #444444 (grey), #000000 (black) or #f00. Default #444444.",
        ))
        .param(
            Param::number("margin")
                .default(36.0)
                .min(0.0)
                .max(400.0)
                .describe("Distance from the page edge in points (0–400; 72 pt = 1 inch). Default 36."),
        )
        .param(
            Param::number("opacity")
                .default(1.0)
                .min(0.05)
                .max(1.0)
                .describe("Text opacity from 0.05 to 1.0 (1 = fully opaque; lower for a faint draft look). Default 1."),
        )
        .param(Param::string("pages").default("all").describe(
            "Which pages to stamp, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfHeaderFooter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-header-footer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a header and footer to a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Stamp header and/or footer text onto an existing PDF. Set the `header` (top of each page) and/or `footer` (bottom) text — provide at least one; use the {page} and {pages} tokens for the page number and page count. Control `header_align`/`footer_align` (left/center/right), which `pages` to stamp (1-based; use '2-' to skip a cover page), and the `font`/`font_size`/`color`/`margin`/`opacity`. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfHeaderFooter {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-header-footer")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let opts = Options {
        header: args.header,
        footer: args.footer,
        header_align: args.header_align,
        footer_align: args.footer_align,
        font: args.font,
        font_size: args.font_size,
        color: args.color,
        margin: args.margin,
        opacity: args.opacity,
        pages: args.pages.clone(),
    };
    let out = gizza_ai_pdf_header_footer_core::add_header_footer(&bytes, &opts)
        .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-header-footer.pdf"))
        .unwrap_or_else(|| "header-footer.pdf".to_string());

    let env = Envelope {
        for_llm: format!(
            "stamped header/footer on pages '{}' of {filename} ({out_len}-byte PDF)",
            opts.pages
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
            r##"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "header": { "type": "string", "default": "", "description": "Text for the header band at the top of each page. Use {page} for the current page number and {pages} for the total page count, e.g. 'Confidential' or 'Page {page} of {pages}'. Leave empty for no header. Provide a header, a footer, or both." },
                    "footer": { "type": "string", "default": "", "description": "Text for the footer band at the bottom of each page. Use {page} for the current page number and {pages} for the total page count. Leave empty for no footer. Provide a header, a footer, or both." },
                    "header_align": { "type": "string", "enum": ["left", "center", "right"], "default": "center", "description": "Horizontal alignment of the header text: left, center or right. Default center." },
                    "footer_align": { "type": "string", "enum": ["left", "center", "right"], "default": "center", "description": "Horizontal alignment of the footer text: left, center or right. Default center." },
                    "font": { "type": "string", "enum": ["helvetica", "times", "courier"], "default": "helvetica", "description": "Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica." },
                    "font_size": { "type": "number", "minimum": 4, "maximum": 144, "default": 10.0, "description": "Font size in points (4–144). Default 10." },
                    "color": { "type": "string", "default": "#444444", "description": "Text colour as a hex code, e.g. #444444 (grey), #000000 (black) or #f00. Default #444444." },
                    "margin": { "type": "number", "minimum": 0, "maximum": 400, "default": 36.0, "description": "Distance from the page edge in points (0–400; 72 pt = 1 inch). Default 36." },
                    "opacity": { "type": "number", "minimum": 0.05, "maximum": 1, "default": 1.0, "description": "Text opacity from 0.05 to 1.0 (1 = fully opaque; lower for a faint draft look). Default 1." },
                    "pages": { "type": "string", "default": "all", "description": "Which pages to stamp, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
