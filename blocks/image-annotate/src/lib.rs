//! gizza-ai/image-annotate — draw arrows, boxes, highlights, and text labels
//! onto an image at given coordinates.
//!
//! Pure-Rust (fontdue + the `image` crate), so unlike the ffmpeg tools it runs
//! on ALL backends including the chat Service Worker. Pipeline: resolve the
//! source image (URL fetch or attachment ref) → `core::render` draws the marks
//! → base64 PNG envelope. Surfaces: chat + CLI. No standalone page (the
//! generated page has no mode for a pure-Rust image-bytes output — same shape as
//! blocks/add-text-to-image and blocks/image-split-overlay).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    annotations: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_stroke")]
    stroke_width: f64,
    #[serde(default = "default_font_size")]
    font_size: f64,
}
fn default_color() -> String { "#ff0000".to_string() }
fn default_stroke() -> f64 { 3.0 }
fn default_font_size() -> f64 { 24.0 }

/// Single-source param descriptor → chat schema (and CLI). `annotations` is a
/// JSON array (passed as a string) so an LLM/CLI user can place many marks in
/// one call; `color`/`stroke_width`/`font_size` are the per-annotation defaults.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("annotations")
                .required()
                .describe(
                    "A JSON array of marks to draw, in order. Each item has a `type` and pixel \
coordinates (top-left origin): box `{\"type\":\"box\",\"x\",\"y\",\"w\",\"h\"}` (hollow \
rectangle), arrow `{\"type\":\"arrow\",\"x1\",\"y1\",\"x2\",\"y2\"}` (arrowhead at x2,y2), \
highlight `{\"type\":\"highlight\",\"x\",\"y\",\"w\",\"h\",\"opacity\":0.35}` (semi-transparent \
wash; opacity 0-1, 1 = solid fill), text `{\"type\":\"text\",\"x\",\"y\",\"text\":\"Label\"}`. \
Any mark may add `color` (#rgb/#rrggbb/#rrggbbaa); box/arrow may add `stroke_width`; text may \
add `font_size`. Example: [{\"type\":\"box\",\"x\":20,\"y\":15,\"w\":120,\"h\":60},\
{\"type\":\"arrow\",\"x1\":200,\"y1\":10,\"x2\":150,\"y2\":45},{\"type\":\"text\",\"x\":22,\
\"y\":92,\"text\":\"Look here\"}]",
                ),
        )
        .param(
            Param::string("color")
                .default("#ff0000")
                .describe("Default mark color as #rgb, #rrggbb, or #rrggbbaa, used when an annotation omits its own `color`. Default #ff0000 (red)."),
        )
        .param(
            Param::number("stroke_width")
                .min(1.0)
                .max(64.0)
                .default(3)
                .describe("Default line/border thickness in pixels for boxes and arrows (1-64). Default 3."),
        )
        .param(
            Param::number("font_size")
                .min(4.0)
                .max(512.0)
                .default(24)
                .describe("Default text label size in pixels (4-512). Default 24."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageAnnotate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-annotate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Draw arrows, boxes, highlights, and text labels onto an image.",
    requires = ["wafer-run/network"],
    skill(
        description = "Draw markup onto an image at exact pixel coordinates and return a PNG. Pass `annotations` as a JSON array of marks — box (hollow rectangle), arrow (line with an arrowhead), highlight (semi-transparent wash), and text (label). Each mark can override the tool-level color; boxes/arrows take a stroke_width, highlights an opacity (0-1), and text a font_size. `color`, `stroke_width`, and `font_size` set the defaults for marks that omit them. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl ImageAnnotate {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("image-annotate")?;
    if args.annotations.trim().is_empty() {
        return Err(SkillError::InvalidArgs(
            "annotations is required (a JSON array of marks)".into(),
        ));
    }
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let png = gizza_ai_image_annotate_core::render(
        &bytes,
        &args.annotations,
        &args.color,
        args.stroke_width as f32,
        args.font_size as f32,
    )
    .map_err(SkillError::InvalidArgs)?;
    let out_len = png.len();

    let encoded = B64.encode(&png);
    let data_url = format!("data:image/png;base64,{encoded}");
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(&in_filename);
    let filename = format!("{stem}-annotated.png");

    let env = Envelope {
        for_llm: format!("annotated {in_filename} ({out_len}-byte PNG: {filename})"),
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
                    "url":          { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "annotations":  { "type": "string", "description": "A JSON array of marks to draw, in order. Each item has a `type` and pixel coordinates (top-left origin): box `{\"type\":\"box\",\"x\",\"y\",\"w\",\"h\"}` (hollow rectangle), arrow `{\"type\":\"arrow\",\"x1\",\"y1\",\"x2\",\"y2\"}` (arrowhead at x2,y2), highlight `{\"type\":\"highlight\",\"x\",\"y\",\"w\",\"h\",\"opacity\":0.35}` (semi-transparent wash; opacity 0-1, 1 = solid fill), text `{\"type\":\"text\",\"x\",\"y\",\"text\":\"Label\"}`. Any mark may add `color` (#rgb/#rrggbb/#rrggbbaa); box/arrow may add `stroke_width`; text may add `font_size`. Example: [{\"type\":\"box\",\"x\":20,\"y\":15,\"w\":120,\"h\":60},{\"type\":\"arrow\",\"x1\":200,\"y1\":10,\"x2\":150,\"y2\":45},{\"type\":\"text\",\"x\":22,\"y\":92,\"text\":\"Look here\"}]" },
                    "color":        { "type": "string", "default": "#ff0000", "description": "Default mark color as #rgb, #rrggbb, or #rrggbbaa, used when an annotation omits its own `color`. Default #ff0000 (red)." },
                    "stroke_width": { "type": "number", "minimum": 1, "maximum": 64, "default": 3, "description": "Default line/border thickness in pixels for boxes and arrows (1-64). Default 3." },
                    "font_size":    { "type": "number", "minimum": 4, "maximum": 512, "default": 24, "description": "Default text label size in pixels (4-512). Default 24." }
                },
                "required": ["annotations"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
