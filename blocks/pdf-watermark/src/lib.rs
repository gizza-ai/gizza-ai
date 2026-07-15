//! gizza-ai/pdf-watermark — stamp a text watermark onto an existing PDF.
//!
//! Loads the source PDF (URL/ref), overlays the SAME text on each selected page
//! — large, faint and rotated (diagonal by default), optionally tiled across the
//! whole page — and returns the new PDF as a base64 envelope. `Input::Document`
//! + the watermark options (text, position, rotation, font/size/colour/opacity,
//! tile, page range). Chat + CLI only (the PDF family has no standalone page
//! surface — binary-in/PDF-out has no page render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_watermark_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    text: String,
    #[serde(default = "d_position")]
    position: String,
    #[serde(default = "d_rotation")]
    rotation: f64,
    #[serde(default = "d_font")]
    font: String,
    #[serde(default = "d_font_size")]
    font_size: f64,
    #[serde(default = "d_color")]
    color: String,
    #[serde(default = "d_opacity")]
    opacity: f64,
    #[serde(default)]
    tile: bool,
    #[serde(default = "d_pages")]
    pages: String,
}

fn d_position() -> String { "center".to_string() }
fn d_rotation() -> f64 { 45.0 }
fn d_font() -> String { "helvetica".to_string() }
fn d_font_size() -> f64 { 48.0 }
fn d_color() -> String { "#808080".to_string() }
fn d_opacity() -> f64 { 0.3 }
fn d_pages() -> String { "all".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("text").required().describe(
            "The watermark text stamped on every selected page, e.g. 'CONFIDENTIAL', 'DRAFT' or 'Acme Corp'. Required.",
        ))
        .param(
            Param::enumv(
                "position",
                [
                    "center", "top-left", "top-center", "top-right", "middle-left", "middle-right",
                    "bottom-left", "bottom-center", "bottom-right",
                ],
            )
            .default("center")
            .describe("Where to anchor the watermark: center (default) or a corner/edge — top/middle/bottom × left/center/right. Ignored when tile is true."),
        )
        .param(
            Param::number("rotation")
                .default(45.0)
                .min(-360.0)
                .max(360.0)
                .describe("Rotation in degrees, counter-clockwise (-360 to 360). 0 = horizontal; 45 (default) or -45 gives the classic diagonal stamp; 90 = vertical."),
        )
        .param(
            Param::enumv("font", ["helvetica", "times", "courier"])
                .default("helvetica")
                .describe("Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica."),
        )
        .param(
            Param::number("font_size")
                .default(48.0)
                .min(4.0)
                .max(288.0)
                .describe("Font size in points (4–288). Watermarks are large — default 48."),
        )
        .param(Param::string("color").default("#808080").describe(
            "Text colour as a hex code, e.g. #808080 (grey, default), #ff0000 (red) or #f00.",
        ))
        .param(
            Param::number("opacity")
                .default(0.3)
                .min(0.05)
                .max(1.0)
                .describe("Opacity from 0.05 (barely visible) to 1.0 (fully opaque). 0.15–0.4 reads as a subtle watermark. Default 0.3."),
        )
        .param(
            Param::boolean("tile")
                .default(false)
                .describe("When true, repeat the watermark in a grid (mosaic) across the whole page instead of a single stamp — harder to crop out. Default false."),
        )
        .param(Param::string("pages").default("all").describe(
            "Which pages to watermark, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfWatermark;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-watermark",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Stamp a text watermark onto every page of a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Stamp a text watermark (CONFIDENTIAL, DRAFT, a company name…) onto every page of a PDF. The same `text` is placed on each selected page — control the `position` (center or a corner/edge), the `rotation` in degrees (45 = classic diagonal), the `font`/`font_size`/`color`/`opacity` (faint by default), whether to `tile` it in a mosaic across the whole page, and which `pages` to mark (1-based; use '2-' to skip a cover page). Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfWatermark {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-watermark")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let opts = Options {
        text: args.text,
        position: args.position,
        rotation: args.rotation,
        font: args.font,
        font_size: args.font_size,
        color: args.color,
        opacity: args.opacity,
        tile: args.tile,
        pages: args.pages.clone(),
    };
    let out = gizza_ai_pdf_watermark_core::add_watermark(&bytes, &opts)
        .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-watermarked.pdf"))
        .unwrap_or_else(|| "watermarked.pdf".to_string());

    let env = Envelope {
        for_llm: format!(
            "watermarked pages '{}' of {filename} with '{}' ({out_len}-byte PDF)",
            opts.pages,
            opts.text.trim()
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
                    "text": { "type": "string", "description": "The watermark text stamped on every selected page, e.g. 'CONFIDENTIAL', 'DRAFT' or 'Acme Corp'. Required." },
                    "position": { "type": "string", "enum": ["center", "top-left", "top-center", "top-right", "middle-left", "middle-right", "bottom-left", "bottom-center", "bottom-right"], "default": "center", "description": "Where to anchor the watermark: center (default) or a corner/edge — top/middle/bottom × left/center/right. Ignored when tile is true." },
                    "rotation": { "type": "number", "minimum": -360, "maximum": 360, "default": 45.0, "description": "Rotation in degrees, counter-clockwise (-360 to 360). 0 = horizontal; 45 (default) or -45 gives the classic diagonal stamp; 90 = vertical." },
                    "font": { "type": "string", "enum": ["helvetica", "times", "courier"], "default": "helvetica", "description": "Built-in font family: helvetica (sans), times (serif) or courier (mono). Default helvetica." },
                    "font_size": { "type": "number", "minimum": 4, "maximum": 288, "default": 48.0, "description": "Font size in points (4–288). Watermarks are large — default 48." },
                    "color": { "type": "string", "default": "#808080", "description": "Text colour as a hex code, e.g. #808080 (grey, default), #ff0000 (red) or #f00." },
                    "opacity": { "type": "number", "minimum": 0.05, "maximum": 1, "default": 0.3, "description": "Opacity from 0.05 (barely visible) to 1.0 (fully opaque). 0.15–0.4 reads as a subtle watermark. Default 0.3." },
                    "tile": { "type": "boolean", "default": false, "description": "When true, repeat the watermark in a grid (mosaic) across the whole page instead of a single stamp — harder to crop out. Default false." },
                    "pages": { "type": "string", "default": "all", "description": "Which pages to watermark, 1-based: 'all' (default), a list/range like '1,3-5', or an open range like '2-' to skip a cover page." }
                },
                "additionalProperties": false,
                "required": ["text"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
