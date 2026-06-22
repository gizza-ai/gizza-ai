//! gizza-ai/svg-to-pdf — convert an SVG drawing into a single-page PDF document.
//! Pure-Rust (resvg rasterizer + lopdf serializer), so it runs on every backend
//! including the chat Service Worker. The PDF is returned as an `application/pdf`
//! data-URL envelope. Surfaces: chat + CLI (document-bytes output → no page, like
//! the svg-to-png / images-to-pdf / qr tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_svg_to_pdf_core::{svg_to_pdf, BgColor, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    svg: String,
    #[serde(default = "default_dpi")]
    dpi: f64,
    #[serde(default = "default_background")]
    background: String,
}

fn default_dpi() -> f64 {
    150.0
}
fn default_background() -> String {
    "white".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svg")
                .required()
                .describe("The SVG markup to convert (the full <svg>…</svg> document)."),
        )
        .param(
            Param::number("dpi")
                .default(150.0)
                .min(24.0)
                .max(1200.0)
                .describe("Rasterization resolution in DPI. Higher = crisper print output and larger file (150 is good for screen, 300 for print)."),
        )
        .param(
            Param::string("background")
                .default("white")
                .describe("Page background colour as 'white' (default) or a hex value (#rgb, #rrggbb). PDF pages are opaque, so transparent SVG areas show as this colour."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Result<Options, String> {
    let background = BgColor::parse(&a.background)?;
    Ok(Options {
        dpi: a.dpi as f32,
        background,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an SVG drawing into a PDF document",
    skill(
        description = "Convert an SVG drawing into a single-page PDF document — no headless browser needed. Pass `svg` as the full <svg>…</svg> markup. The PDF page is sized to the SVG's intrinsic dimensions (so it prints at real-world size), and the artwork is embedded at the chosen `dpi` (150 default; raise to 300 for print, lower for a smaller file). `background` ('white' or a hex colour like #ffffff) fills the opaque PDF page behind the SVG. Note: <text> needs a font; convert text to paths in your editor for pixel-perfect labels (shapes/paths render exactly). Returns a PDF. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("svg-to-pdf")?;
    let opts = build_options(&args).map_err(SkillError::InvalidArgs)?;
    let bytes = svg_to_pdf(&args.svg, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &bytes,
        "application/pdf",
        "drawing.pdf".to_string(),
        format!("Converted SVG to PDF ({} bytes)", bytes.len()),
        MAX_OUTPUT_BYTES,
    )
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
                    "svg":        { "type": "string", "description": "The SVG markup to convert (the full <svg>…</svg> document)." },
                    "dpi":        { "type": "number", "minimum": 24, "maximum": 1200, "default": 150.0, "description": "Rasterization resolution in DPI. Higher = crisper print output and larger file (150 is good for screen, 300 for print)." },
                    "background": { "type": "string", "default": "white", "description": "Page background colour as 'white' (default) or a hex value (#rgb, #rrggbb). PDF pages are opaque, so transparent SVG areas show as this colour." }
                },
                "required": ["svg"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
