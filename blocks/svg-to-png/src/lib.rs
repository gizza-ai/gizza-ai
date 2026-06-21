//! gizza-ai/svg-to-png — rasterize an SVG document into a PNG or JPEG at a chosen
//! resolution. Pure-Rust (resvg + tiny-skia), so it runs on every backend
//! including the chat Service Worker. The image is returned as an `image/png`
//! or `image/jpeg` data-URL envelope. Surfaces: chat + CLI (image-bytes output →
//! no page, like the QR / chart / latex-math-to-svg tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_svg_to_png_core::{rasterize, BgColor, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    svg: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_quality")]
    quality: u32,
    #[serde(default = "default_background")]
    background: String,
}

fn default_format() -> String {
    "png".to_string()
}
fn default_scale() -> f64 {
    1.0
}
fn default_quality() -> u32 {
    90
}
fn default_background() -> String {
    "transparent".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svg")
                .required()
                .describe("The SVG markup to rasterize (the full <svg>…</svg> document)."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output raster format: 'png' (lossless, keeps transparency) or 'jpeg' (smaller, no transparency)."),
        )
        .param(
            Param::integer("width")
                .min(0.0)
                .max(8000.0)
                .describe("Output width in px. 0 (default) derives the width from height/scale and the SVG's aspect ratio."),
        )
        .param(
            Param::integer("height")
                .min(0.0)
                .max(8000.0)
                .describe("Output height in px. 0 (default) derives the height from width/scale and the SVG's aspect ratio."),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .min(0.01)
                .max(50.0)
                .describe("Multiplier applied to the SVG's intrinsic size when neither width nor height is set (1.0 = native size, 2.0 = 2x)."),
        )
        .param(
            Param::integer("quality")
                .default(90)
                .min(1.0)
                .max(100.0)
                .describe("JPEG quality 1–100 (ignored for PNG). Higher = better quality, larger file."),
        )
        .param(
            Param::string("background")
                .default("transparent")
                .describe("Background colour as 'transparent' or a hex value (#rgb, #rrggbb, #rrggbbaa). PNG keeps alpha; JPEG composites onto this colour (white if transparent)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Result<Options, String> {
    let format = OutputFormat::parse(&a.format)?;
    let background = BgColor::parse(&a.background)?;
    let quality = u8::try_from(a.quality).map_err(|_| "quality must be between 1 and 100".to_string())?;
    Ok(Options {
        format,
        width: a.width,
        height: a.height,
        scale: a.scale as f32,
        quality,
        background,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-to-png",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rasterize an SVG into a PNG or JPEG",
    skill(
        description = "Rasterize an SVG document into a PNG or JPEG raster image at a chosen resolution — no headless browser needed. Pass `svg` as the full <svg>…</svg> markup. `format` is 'png' (lossless, keeps transparency) or 'jpeg' (smaller, no transparency). Size it with `width`/`height` in px (set one to keep the SVG's aspect ratio, or both to force an exact size); if neither is set, `scale` multiplies the SVG's intrinsic size (1.0 = native). `quality` (1–100) controls JPEG compression; `background` ('transparent' or a hex colour like #ffffff) fills behind the SVG — JPEG, which has no alpha, composites onto it (white if transparent). Note: <text> needs a font; convert text to paths in your editor for pixel-perfect labels (shapes/paths render exactly). Returns an image. Runs locally on the device.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("svg-to-png")?;
    let opts = build_options(&args).map_err(SkillError::InvalidArgs)?;
    let bytes = rasterize(&args.svg, &opts).map_err(SkillError::InvalidArgs)?;
    let ext = opts.format.extension();
    build_media_envelope(
        &bytes,
        opts.format.mime(),
        format!("image.{ext}"),
        format!("Rasterized SVG as {} ({} bytes)", ext.to_ascii_uppercase(), bytes.len()),
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
                    "svg":        { "type": "string", "description": "The SVG markup to rasterize (the full <svg>…</svg> document)." },
                    "format":     { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output raster format: 'png' (lossless, keeps transparency) or 'jpeg' (smaller, no transparency)." },
                    "width":      { "type": "integer", "minimum": 0, "maximum": 8000, "description": "Output width in px. 0 (default) derives the width from height/scale and the SVG's aspect ratio." },
                    "height":     { "type": "integer", "minimum": 0, "maximum": 8000, "description": "Output height in px. 0 (default) derives the height from width/scale and the SVG's aspect ratio." },
                    "scale":      { "type": "number", "minimum": 0.01, "maximum": 50, "default": 1.0, "description": "Multiplier applied to the SVG's intrinsic size when neither width nor height is set (1.0 = native size, 2.0 = 2x)." },
                    "quality":    { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "JPEG quality 1–100 (ignored for PNG). Higher = better quality, larger file." },
                    "background": { "type": "string", "default": "transparent", "description": "Background colour as 'transparent' or a hex value (#rgb, #rrggbb, #rrggbbaa). PNG keeps alpha; JPEG composites onto this colour (white if transparent)." }
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
