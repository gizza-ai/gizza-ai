//! gizza-ai/text-image-card — render a quote or short text onto a styled,
//! shareable PNG card.
//!
//! Pure-Rust (fontdue + the `image` crate), so unlike the ffmpeg tools it runs
//! on ALL backends including the chat Service Worker. Pipeline: pick a theme +
//! layout → core::render draws the wrapped text (and optional author byline)
//! onto a gradient/solid background → PNG envelope. Surfaces: chat + CLI. No
//! standalone page (the generated page has no mode for a pure-Rust image-bytes
//! output — that mode is only for ffmpeg argv tools), so this is chat + CLI like
//! blocks/qr-code-generator / blocks/add-text-to-image.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_text_image_card_core::{render, Card};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    text: String,
    #[serde(default)]
    author: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    text_color: String,
    #[serde(default)]
    background_color: String,
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_width() -> u32 {
    1080
}
fn default_height() -> u32 {
    1080
}
fn default_font_size() -> f64 {
    64.0
}
fn default_align() -> String {
    "center".to_string()
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The quote or short text to render onto the card (use \\n for hard line breaks; it word-wraps automatically)."),
        )
        .param(
            Param::string("author")
                .describe("Optional attribution shown as a smaller byline under the text (e.g. a person's name)."),
        )
        .param(
            Param::enumv("theme", ["dark", "light", "sunset", "ocean", "forest", "grape"])
                .default("dark")
                .describe("Colour theme (background gradient + text colour): dark, light, sunset, ocean, forest, or grape."),
        )
        .param(
            Param::integer("width")
                .default(1080)
                .min(200.0)
                .max(4000.0)
                .describe("Card width in pixels (200-4000; default 1080)."),
        )
        .param(
            Param::integer("height")
                .default(1080)
                .min(200.0)
                .max(4000.0)
                .describe("Card height in pixels (200-4000; default 1080)."),
        )
        .param(
            Param::number("font_size")
                .default(64)
                .min(8.0)
                .describe("Starting body font size in pixels (default 64). Auto-shrinks if the text would not fit."),
        )
        .param(
            Param::enumv("align", ["left", "center", "right"])
                .default("center")
                .describe("Horizontal text alignment: left, center, or right (default center)."),
        )
        .param(
            Param::string("text_color")
                .describe("Override the theme text colour as #rgb or #rrggbb hex (optional)."),
        )
        .param(
            Param::string("background_color")
                .describe("Override the theme background with a solid colour as #rgb or #rrggbb hex; disables the gradient (optional)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextImageCard;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-image-card",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a quote onto a styled shareable image card (PNG)",
    skill(
        description = "Render a quote or short piece of text onto a styled, shareable image card and return a PNG. Set text (the quote — use \\n for hard line breaks; it word-wraps to fit), an optional author byline, a theme (dark, light, sunset, ocean, forest, grape — a coloured background gradient + matching text colour), the card width/height in pixels (defaults 1080x1080, ideal for social posts), font_size (auto-shrinks if the text would not fit), align (left/center/right), and optional text_color / background_color hex overrides. Returns an image. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl TextImageCard {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("text-image-card")?;
    let card = Card {
        text: &args.text,
        author: &args.author,
        theme: &args.theme,
        width: args.width,
        height: args.height,
        font_size: args.font_size as f32,
        align: &args.align,
        text_color: &args.text_color,
        bg_color: &args.background_color,
    };
    let png = render(&card).map_err(SkillError::InvalidArgs)?;
    let n = png.len();
    build_media_envelope(
        &png,
        "image/png",
        "card.png".to_string(),
        format!("text image card ({} theme, {n}-byte PNG)", args.theme),
        MAX_OUTPUT_BYTES,
    )
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
                    "text":             { "type": "string", "description": "The quote or short text to render onto the card (use \\n for hard line breaks; it word-wraps automatically)." },
                    "author":           { "type": "string", "description": "Optional attribution shown as a smaller byline under the text (e.g. a person's name)." },
                    "theme":            { "type": "string", "enum": ["dark", "light", "sunset", "ocean", "forest", "grape"], "default": "dark", "description": "Colour theme (background gradient + text colour): dark, light, sunset, ocean, forest, or grape." },
                    "width":            { "type": "integer", "default": 1080, "minimum": 200, "maximum": 4000, "description": "Card width in pixels (200-4000; default 1080)." },
                    "height":           { "type": "integer", "default": 1080, "minimum": 200, "maximum": 4000, "description": "Card height in pixels (200-4000; default 1080)." },
                    "font_size":        { "type": "number", "default": 64, "minimum": 8, "description": "Starting body font size in pixels (default 64). Auto-shrinks if the text would not fit." },
                    "align":            { "type": "string", "enum": ["left", "center", "right"], "default": "center", "description": "Horizontal text alignment: left, center, or right (default center)." },
                    "text_color":       { "type": "string", "description": "Override the theme text colour as #rgb or #rrggbb hex (optional)." },
                    "background_color": { "type": "string", "description": "Override the theme background with a solid colour as #rgb or #rrggbb hex; disables the gradient (optional)." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
