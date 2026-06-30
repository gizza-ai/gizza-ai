//! gizza-ai/text-banner-image — render a short headline into a wide, stylized
//! PNG banner (gradient/accent background, drop shadow, outline).
//!
//! Pure-Rust (fontdue + the `image` crate), so unlike the ffmpeg tools it runs
//! on ALL backends including the chat Service Worker. Pipeline: parse the params
//! → core::render paints the gradient + accent stripe, word-wraps + auto-shrinks
//! the headline, draws it with the optional shadow/outline → PNG envelope.
//! Surfaces: chat + CLI. No standalone page (a pure-Rust image-bytes output has
//! no page render mode — same as blocks/text-image-card / qr-code-generator).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_text_banner_image_core::{render, Banner};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    text: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_bg")]
    bg_color: String,
    #[serde(default = "default_text_color")]
    text_color: String,
    #[serde(default = "default_accent")]
    accent_color: String,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    font_size: f64,
    #[serde(default = "default_true")]
    shadow: bool,
    #[serde(default)]
    outline: bool,
}
fn default_width() -> u32 {
    1200
}
fn default_height() -> u32 {
    400
}
fn default_bg() -> String {
    "#111827".to_string()
}
fn default_text_color() -> String {
    "#ffffff".to_string()
}
fn default_accent() -> String {
    "#60a5fa".to_string()
}
fn default_align() -> String {
    "center".to_string()
}
fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The headline / banner text to render (use \\n for hard line breaks; it word-wraps and auto-shrinks to fit)."),
        )
        .param(
            Param::integer("width")
                .default(1200)
                .min(200.0)
                .max(4000.0)
                .describe("Banner width in pixels (200-4000; default 1200)."),
        )
        .param(
            Param::integer("height")
                .default(400)
                .min(100.0)
                .max(4000.0)
                .describe("Banner height in pixels (100-4000; default 400)."),
        )
        .param(
            Param::string("bg_color")
                .default("#111827")
                .describe("Background base colour as #rgb or #rrggbb hex (default #111827). The background is a subtle diagonal gradient from this colour toward the accent."),
        )
        .param(
            Param::string("text_color")
                .default("#ffffff")
                .describe("Headline text colour as #rgb or #rrggbb hex (default #ffffff)."),
        )
        .param(
            Param::string("accent_color")
                .default("#60a5fa")
                .describe("Accent colour as #rgb or #rrggbb hex (default #60a5fa) — used for the left stripe, the gradient tint, and the underline."),
        )
        .param(
            Param::enumv("align", ["center", "left", "right"])
                .default("center")
                .describe("Horizontal text alignment: center, left, or right (default center)."),
        )
        .param(
            Param::number("font_size")
                .default(0)
                .min(0.0)
                .describe("Font size in pixels, or 0 to auto-size to fit the banner (default 0). A fixed size still auto-shrinks if the text would overflow."),
        )
        .param(
            Param::boolean("shadow")
                .default(true)
                .describe("Draw a soft drop shadow behind the text (default true)."),
        )
        .param(
            Param::boolean("outline")
                .default(false)
                .describe("Draw a dark outline around the text for extra contrast (default false)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextBannerImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-banner-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a headline into a stylized PNG banner (gradient, shadow, outline)",
    skill(
        description = "Render a short headline into a wide, stylized banner image and return a PNG. Set text (the headline — use \\n for hard line breaks; it word-wraps and auto-shrinks to fit), the banner width/height in pixels (defaults 1200x400), bg_color / text_color / accent_color as #rgb or #rrggbb hex (the background is a subtle diagonal gradient from bg_color toward the accent, with a left accent stripe + underline), align (center/left/right), font_size (0 = auto-fit), and shadow / outline toggles for a drop shadow and a dark text outline. Returns an image. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl TextBannerImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("text-banner-image")?;
    let banner = Banner {
        text: &args.text,
        width: args.width,
        height: args.height,
        bg_color: &args.bg_color,
        text_color: &args.text_color,
        accent_color: &args.accent_color,
        align: &args.align,
        font_size: args.font_size as f32,
        shadow: args.shadow,
        outline: args.outline,
    };
    let png = render(&banner).map_err(SkillError::InvalidArgs)?;
    let n = png.len();
    build_media_envelope(
        &png,
        "image/png",
        "banner.png".to_string(),
        format!("text banner ({}x{}, {n}-byte PNG)", args.width, args.height),
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
                    "text":         { "type": "string", "description": "The headline / banner text to render (use \\n for hard line breaks; it word-wraps and auto-shrinks to fit)." },
                    "width":        { "type": "integer", "default": 1200, "minimum": 200, "maximum": 4000, "description": "Banner width in pixels (200-4000; default 1200)." },
                    "height":       { "type": "integer", "default": 400, "minimum": 100, "maximum": 4000, "description": "Banner height in pixels (100-4000; default 400)." },
                    "bg_color":     { "type": "string", "default": "#111827", "description": "Background base colour as #rgb or #rrggbb hex (default #111827). The background is a subtle diagonal gradient from this colour toward the accent." },
                    "text_color":   { "type": "string", "default": "#ffffff", "description": "Headline text colour as #rgb or #rrggbb hex (default #ffffff)." },
                    "accent_color": { "type": "string", "default": "#60a5fa", "description": "Accent colour as #rgb or #rrggbb hex (default #60a5fa) — used for the left stripe, the gradient tint, and the underline." },
                    "align":        { "type": "string", "enum": ["center", "left", "right"], "default": "center", "description": "Horizontal text alignment: center, left, or right (default center)." },
                    "font_size":    { "type": "number", "default": 0, "minimum": 0, "description": "Font size in pixels, or 0 to auto-size to fit the banner (default 0). A fixed size still auto-shrinks if the text would overflow." },
                    "shadow":       { "type": "boolean", "default": true, "description": "Draw a soft drop shadow behind the text (default true)." },
                    "outline":      { "type": "boolean", "default": false, "description": "Draw a dark outline around the text for extra contrast (default false)." }
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
