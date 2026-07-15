//! gizza-ai/add-text-to-image — overlay caption/watermark text on an image.
//!
//! Pure-Rust (fontdue + the `image` crate), so unlike the ffmpeg tools it runs
//! on ALL backends including the chat Service Worker. Pipeline: resolve the
//! source image (URL fetch or attachment ref) → `core::render` draws the text →
//! base64 PNG envelope. Surfaces: chat + CLI. No standalone page (the generated
//! page has no mode for a pure-Rust image-bytes output — the ffmpeg page mode is
//! for ffmpeg argv tools), so this is chat + CLI like blocks/code-screenshot.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    text: String,
    #[serde(default = "default_x")]
    x: i64,
    #[serde(default = "default_y")]
    y: i64,
    #[serde(default = "default_size")]
    font_size: f64,
    #[serde(default = "default_color")]
    color: String,
}
fn default_x() -> i64 { 10 }
fn default_y() -> i64 { 10 }
fn default_size() -> f64 { 32.0 }
fn default_color() -> String { "#ffffff".to_string() }

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::string("text").required().describe("The text to overlay (use \\n for multiple lines)."))
        .param(Param::integer("x").default(10).min(0.0).describe("Left position of the text in pixels (default 10)."))
        .param(Param::integer("y").default(10).min(0.0).describe("Top position of the text in pixels (default 10)."))
        .param(Param::number("font_size").default(32).min(4.0).describe("Font size in pixels (default 32)."))
        .param(Param::string("color").default("#ffffff").describe("Text colour as #rrggbb (default #ffffff)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AddTextToImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/add-text-to-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Overlay caption/watermark text on an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Overlay custom text (a caption, watermark, or meme line) onto an image and return a PNG. Set x/y for the top-left text position in pixels, font_size in pixels, and color as #rrggbb. Use \\n in text for multiple lines. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl AddTextToImage {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("add-text-to-image")?;
    if args.text.is_empty() {
        return Err(SkillError::InvalidArgs("text is required".into()));
    }
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let png = gizza_ai_add_text_to_image_core::render(
        &bytes, &args.text, args.x, args.y, args.font_size as f32, &args.color,
    )
    .map_err(SkillError::InvalidArgs)?;
    let out_len = png.len();

    let encoded = B64.encode(&png);
    let data_url = format!("data:image/png;base64,{encoded}");
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(&in_filename);
    let filename = format!("{stem}-captioned.png");

    let env = Envelope {
        for_llm: format!("added text to {in_filename} ({out_len}-byte PNG: {filename})"),
        for_ui: ForUi { data_url, mime: "image/png".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "text":      { "type": "string", "description": "The text to overlay (use \\n for multiple lines)." },
                    "x":         { "type": "integer", "minimum": 0, "default": 10, "description": "Left position of the text in pixels (default 10)." },
                    "y":         { "type": "integer", "minimum": 0, "default": 10, "description": "Top position of the text in pixels (default 10)." },
                    "font_size": { "type": "number", "minimum": 4, "default": 32, "description": "Font size in pixels (default 32)." },
                    "color":     { "type": "string", "default": "#ffffff", "description": "Text colour as #rrggbb (default #ffffff)." }
                },
                "required": ["text"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
