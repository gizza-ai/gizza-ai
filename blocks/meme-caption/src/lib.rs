//! gizza-ai/meme-caption — add a classic top/bottom impact-style caption to an image.
//!
//! Pure-Rust (fontdue + the `image` crate), so unlike the ffmpeg tools it runs
//! on ALL backends including the chat Service Worker. Pipeline: resolve the
//! source image (URL fetch or attachment ref) → `core::caption` draws the meme
//! text → base64 PNG envelope. Surfaces: chat + CLI. No standalone page (the
//! generated page has no mode for a pure-Rust image-bytes output — the ffmpeg
//! page mode is for ffmpeg argv tools), so this is chat + CLI like
//! blocks/add-text-to-image / blocks/code-screenshot.
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
    #[serde(default)]
    top: String,
    #[serde(default)]
    bottom: String,
    #[serde(default = "default_uppercase")]
    uppercase: bool,
    #[serde(default)]
    text_color: String,
    #[serde(default)]
    outline_color: String,
}
fn default_uppercase() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("top")
                .describe("Top caption text (optional). Rendered in uppercase, centered."),
        )
        .param(
            Param::string("bottom")
                .describe("Bottom caption text (optional). Rendered in uppercase, centered."),
        )
        .param(
            Param::boolean("uppercase")
                .default(true)
                .describe("Uppercase the caption text for the classic meme look (default true). Set false to keep the text exactly as written."),
        )
        .param(
            Param::string("text_color")
                .default("#ffffff")
                .describe("Caption fill colour as #rrggbb (default #ffffff white)."),
        )
        .param(
            Param::string("outline_color")
                .default("#000000")
                .describe("Caption outline/stroke colour as #rrggbb (default #000000 black)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MemeCaption;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/meme-caption",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a classic top/bottom impact-style caption to an image",
    requires = ["wafer-run/network"],
    skill(
        description = "Add a classic top and/or bottom meme caption to an image and return a PNG. The text is rendered impact-style: bold letters with an outline, centered horizontally, auto-sized to the image width (long captions wrap), placed near the top and bottom edges, and uppercased by default. Set top and/or bottom (at least one is required); set uppercase=false to keep the text exactly as written, and text_color/outline_color as #rrggbb to recolour the fill and stroke (default white text with a black outline). Provide the image as either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl MemeCaption {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("meme-caption")?;
    if args.top.trim().is_empty() && args.bottom.trim().is_empty() {
        return Err(SkillError::InvalidArgs(
            "provide top and/or bottom caption text".into(),
        ));
    }
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let png = gizza_ai_meme_caption_core::caption(
        &bytes,
        &args.top,
        &args.bottom,
        args.uppercase,
        &args.text_color,
        &args.outline_color,
    )
    .map_err(SkillError::InvalidArgs)?;
    let out_len = png.len();

    let encoded = B64.encode(&png);
    let data_url = format!("data:image/png;base64,{encoded}");
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(&in_filename);
    let filename = format!("{stem}-meme.png");

    let env = Envelope {
        for_llm: format!("added meme caption to {in_filename} ({out_len}-byte PNG: {filename})"),
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "top":    { "type": "string", "description": "Top caption text (optional). Rendered in uppercase, centered." },
                    "bottom": { "type": "string", "description": "Bottom caption text (optional). Rendered in uppercase, centered." },
                    "uppercase": { "type": "boolean", "default": true, "description": "Uppercase the caption text for the classic meme look (default true). Set false to keep the text exactly as written." },
                    "text_color": { "type": "string", "default": "#ffffff", "description": "Caption fill colour as #rrggbb (default #ffffff white)." },
                    "outline_color": { "type": "string", "default": "#000000", "description": "Caption outline/stroke colour as #rrggbb (default #000000 black)." }
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
