//! gizza-ai/svg-placeholder-generator — create a placeholder image as a scalable
//! SVG at a chosen size, with a centred label (the dimensions by default, or
//! custom text), a background colour, and a text colour.
//!
//! Pure-Rust (a hand-built SVG string), so it runs on ALL backends incl. the
//! chat Service Worker. The SVG is wrapped as an `image/svg+xml` data-URL
//! envelope. Surfaces: chat + CLI (image-bytes output → no page, like the
//! gradient-image-generator / qr-code-generator tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_svg_placeholder_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default)]
    text: String,
    #[serde(default = "default_bg_color")]
    bg_color: String,
    #[serde(default)]
    text_color: String,
    /// Font size in px. f64 (wasm BigInt gotcha); 0 = auto-fit.
    #[serde(default)]
    font_size: f64,
    #[serde(default = "default_font_family")]
    font_family: String,
}
fn default_width() -> u32 {
    600
}
fn default_height() -> u32 {
    400
}
fn default_bg_color() -> String {
    "#cccccc".to_string()
}
fn default_font_family() -> String {
    "sans-serif".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::integer("width")
                .default(600)
                .min(1.0)
                .max(4096.0)
                .describe("Placeholder width in pixels (1-4096)."),
        )
        .param(
            Param::integer("height")
                .default(400)
                .min(1.0)
                .max(4096.0)
                .describe("Placeholder height in pixels (1-4096)."),
        )
        .param(Param::string("text").describe(
            "The centred label text. Leave empty to auto-label with the dimensions, e.g. \"600×400\".",
        ))
        .param(Param::string("bg_color").default("#cccccc").describe(
            "Background colour as a hex value (#rgb, #rgba, #rrggbb, or #rrggbbaa — alpha is ignored). Defaults to a light grey.",
        ))
        .param(Param::string("text_color").describe(
            "Label colour as a hex value (#rgb/#rrggbb…). Leave empty to auto-pick a readable colour (dark text on light backgrounds, white on dark).",
        ))
        .param(
            Param::number("font_size")
                .default(0.0)
                .min(0.0)
                .max(4096.0)
                .describe("Label font size in pixels. 0 (the default) auto-fits the label to the box."),
        )
        .param(Param::string("font_family").default("sans-serif").describe(
            "CSS font-family for the label, e.g. \"sans-serif\", \"serif\", or \"Georgia, serif\".",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-placeholder-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a placeholder SVG image at a given size with an optional label",
    skill(
        description = "Generate a placeholder image as a scalable SVG at a chosen size, for mockups, wireframes, design comps, and dummy content. width and height set the size in pixels (1-4096). text is the centred label — leave it empty to auto-label with the dimensions (e.g. \"600×400\"). bg_color sets the background (a hex colour; defaults to light grey) and text_color the label (a hex colour; leave empty to auto-pick a readable dark/white colour from the background). font_size sets the label size in px (0 = auto-fit) and font_family the typeface. Returns a scalable SVG image. Runs locally — nothing leaves the device.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("svg-placeholder-generator")?;
    let g = generate(
        args.width,
        args.height,
        &args.text,
        &args.bg_color,
        &args.text_color,
        args.font_size,
        &args.font_family,
    )
    .map_err(SkillError::InvalidArgs)?;
    let bytes = g.svg.into_bytes();
    let n = bytes.len();
    build_media_envelope(
        &bytes,
        "image/svg+xml",
        "placeholder.svg".to_string(),
        format!("placeholder SVG {}x{} (\"{}\", {n} bytes)", g.width, g.height, g.label),
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
                    "width": { "type": "integer", "default": 600, "minimum": 1, "maximum": 4096, "description": "Placeholder width in pixels (1-4096)." },
                    "height": { "type": "integer", "default": 400, "minimum": 1, "maximum": 4096, "description": "Placeholder height in pixels (1-4096)." },
                    "text": { "type": "string", "description": "The centred label text. Leave empty to auto-label with the dimensions, e.g. \"600×400\"." },
                    "bg_color": { "type": "string", "default": "#cccccc", "description": "Background colour as a hex value (#rgb, #rgba, #rrggbb, or #rrggbbaa — alpha is ignored). Defaults to a light grey." },
                    "text_color": { "type": "string", "description": "Label colour as a hex value (#rgb/#rrggbb…). Leave empty to auto-pick a readable colour (dark text on light backgrounds, white on dark)." },
                    "font_size": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 4096, "description": "Label font size in pixels. 0 (the default) auto-fits the label to the box." },
                    "font_family": { "type": "string", "default": "sans-serif", "description": "CSS font-family for the label, e.g. \"sans-serif\", \"serif\", or \"Georgia, serif\"." }
                },
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
