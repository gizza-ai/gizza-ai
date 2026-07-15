//! gizza-ai/pdf-page-numbers — stamp page numbers onto an existing PDF.
//!
//! Loads the source PDF (URL/ref), overlays a page-number label on each selected
//! page, and returns the new PDF as a base64 envelope. `Input::Document` + the
//! stamp options (format template, position, numbering style, start number, page
//! range, font/size/colour/margin/opacity). Chat + CLI only (the PDF family has
//! no standalone page surface — binary-in/PDF-out has no page render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_page_numbers_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_format")]
    format: String,
    #[serde(default = "d_position")]
    position: String,
    #[serde(default = "d_style")]
    style: String,
    #[serde(default = "d_start")]
    start_number: i64,
    #[serde(default = "d_pages")]
    pages: String,
    #[serde(default = "d_font")]
    font: String,
    #[serde(default = "d_font_size")]
    font_size: f64,
    #[serde(default = "d_margin")]
    margin: f64,
    #[serde(default = "d_color")]
    color: String,
    #[serde(default = "d_opacity")]
    opacity: f64,
}

fn d_format() -> String { "{n}".to_string() }
fn d_position() -> String { "bottom-center".to_string() }
fn d_style() -> String { "decimal".to_string() }
fn d_start() -> i64 { 1 }
fn d_pages() -> String { "all".to_string() }
fn d_font() -> String { "helvetica".to_string() }
fn d_font_size() -> f64 { 12.0 }
fn d_margin() -> f64 { 36.0 }
fn d_color() -> String { "#000000".to_string() }
fn d_opacity() -> f64 { 1.0 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("format").default("{n}").describe(
            "Label template. Use {n} for the page's number and {total} for the last number printed, e.g. '{n}', 'Page {n} of {total}', or '{n} / {total}'. Default '{n}'.",
        ))
        .param(
            Param::enumv(
                "position",
                ["bottom-center", "bottom-left", "bottom-right", "top-center", "top-left", "top-right"],
            )
            .default("bottom-center")
            .describe("Where to place the number: bottom or top of the page, left/center/right. Default bottom-center."),
        )
        .param(
            Param::enumv("style", ["decimal", "roman-lower", "roman-upper", "alpha-lower", "alpha-upper"])
                .default("decimal")
                .describe("Numbering style: decimal (1,2,3), roman-lower (i,ii,iii), roman-upper (I,II,III), alpha-lower (a,b,c) or alpha-upper (A,B,C). Default decimal."),
        )
        .param(
            Param::integer("start_number")
                .default(1)
                .describe("The number printed on the first stamped page; later pages increment from it. Default 1."),
        )
        .param(Param::string("pages").default("all").describe(
            "Which pages to number, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page.",
        ))
        .param(
            Param::enumv("font", ["helvetica", "times", "courier"])
                .default("helvetica")
                .describe("Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica."),
        )
        .param(
            Param::number("font_size")
                .default(12.0)
                .min(4.0)
                .max(144.0)
                .describe("Font size in points (4–144). Default 12."),
        )
        .param(
            Param::number("margin")
                .default(36.0)
                .min(0.0)
                .max(400.0)
                .describe("Distance from the page edge in points (0–400; 72 pt = 1 inch). Default 36."),
        )
        .param(Param::string("color").default("#000000").describe(
            "Text colour as a hex code, e.g. #000000 (black), #808080 (grey) or #f00. Default #000000.",
        ))
        .param(
            Param::number("opacity")
                .default(1.0)
                .min(0.05)
                .max(1.0)
                .describe("Text opacity from 0.05 to 1.0 (1 = fully opaque; lower for a faint watermark-style number). Default 1."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfPageNumbers;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-page-numbers",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add page numbers to a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Stamp page numbers onto an existing PDF. Control the label with a `format` template ({n} = page number, {total} = last number printed), the `position` (bottom/top × left/center/right), the numbering `style` (decimal, roman, or letters), the `start_number`, which `pages` to number (1-based; use '2-' to skip a cover page), and the `font`/`font_size`/`color`/`margin`/`opacity`. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfPageNumbers {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-page-numbers")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let opts = Options {
        format: args.format,
        position: args.position,
        style: args.style,
        start_number: args.start_number,
        pages: args.pages.clone(),
        font: args.font,
        font_size: args.font_size,
        margin: args.margin,
        color: args.color,
        opacity: args.opacity,
    };
    let out = gizza_ai_pdf_page_numbers_core::add_page_numbers(&bytes, &opts)
        .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-numbered.pdf"))
        .unwrap_or_else(|| "numbered.pdf".to_string());

    let env = Envelope {
        for_llm: format!(
            "numbered pages '{}' of {filename} ({out_len}-byte PDF)",
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
                    "format": { "type": "string", "default": "{n}", "description": "Label template. Use {n} for the page's number and {total} for the last number printed, e.g. '{n}', 'Page {n} of {total}', or '{n} / {total}'. Default '{n}'." },
                    "position": { "type": "string", "enum": ["bottom-center", "bottom-left", "bottom-right", "top-center", "top-left", "top-right"], "default": "bottom-center", "description": "Where to place the number: bottom or top of the page, left/center/right. Default bottom-center." },
                    "style": { "type": "string", "enum": ["decimal", "roman-lower", "roman-upper", "alpha-lower", "alpha-upper"], "default": "decimal", "description": "Numbering style: decimal (1,2,3), roman-lower (i,ii,iii), roman-upper (I,II,III), alpha-lower (a,b,c) or alpha-upper (A,B,C). Default decimal." },
                    "start_number": { "type": "integer", "default": 1, "description": "The number printed on the first stamped page; later pages increment from it. Default 1." },
                    "pages": { "type": "string", "default": "all", "description": "Which pages to number, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page." },
                    "font": { "type": "string", "enum": ["helvetica", "times", "courier"], "default": "helvetica", "description": "Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica." },
                    "font_size": { "type": "number", "minimum": 4, "maximum": 144, "default": 12.0, "description": "Font size in points (4–144). Default 12." },
                    "margin": { "type": "number", "minimum": 0, "maximum": 400, "default": 36.0, "description": "Distance from the page edge in points (0–400; 72 pt = 1 inch). Default 36." },
                    "color": { "type": "string", "default": "#000000", "description": "Text colour as a hex code, e.g. #000000 (black), #808080 (grey) or #f00. Default #000000." },
                    "opacity": { "type": "number", "minimum": 0.05, "maximum": 1, "default": 1.0, "description": "Text opacity from 0.05 to 1.0 (1 = fully opaque; lower for a faint watermark-style number). Default 1." }
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
