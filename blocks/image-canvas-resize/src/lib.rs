//! gizza-ai/image-canvas-resize — change image canvas dimensions without scaling
//! the source pixels. Returns a PNG of exactly the requested width × height.
//! Pure Rust (image crate) — runs on all backends incl. the chat SW. Surfaces:
//! chat + CLI (image input + image-bytes output → no page, like image-contain-fit).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_canvas_resize_core::{canvas_resize, parse_color, Anchor, ANCHORS};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    width: u32,
    height: u32,
    #[serde(default = "default_anchor")]
    anchor: String,
    #[serde(default = "default_fill")]
    fill: String,
}

fn default_anchor() -> String {
    "center".to_string()
}

fn default_fill() -> String {
    "#ffffff".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("width")
                .required()
                .min(1.0)
                .describe("Output canvas width in pixels. Source pixels are not scaled."),
        )
        .param(
            Param::integer("height")
                .required()
                .min(1.0)
                .describe("Output canvas height in pixels. Source pixels are not scaled."),
        )
        .param(
            Param::enumv("anchor", ANCHORS)
                .default("center")
                .describe("Where to pin the source image on the new canvas: controls padding when growing and what gets cropped when shrinking."),
        )
        .param(
            Param::string("fill")
                .default("#ffffff")
                .describe("Fill colour for newly exposed canvas: #rgb, #rgba, #rrggbb, #rrggbbaa, white, black, red, green, blue, gray, or transparent."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-canvas-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change an image canvas size without scaling pixels, padding or cropping by anchor",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Change an image canvas to an exact width x height without scaling the source pixels. If the canvas grows, new margins are filled with fill; if it shrinks, source pixels are cropped. anchor chooses where the source is pinned (center, top, bottom, left, right, top-left, top-right, bottom-left, bottom-right). fill accepts hex colours (#rgb, #rgba, #rrggbb, #rrggbbaa), common names, or transparent. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-canvas-resize")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid image-canvas-resize args: width and height must be > 0".into(),
        ));
    }
    let anchor = Anchor::parse(&args.anchor).map_err(SkillError::InvalidArgs)?;
    let fill = parse_color(&args.fill).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = canvas_resize(&bytes, args.width, args.height, anchor, fill)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "canvas-resize.png".to_string(),
        format!(
            "changed canvas to {}x{} (anchor {}, fill {}, {} bytes PNG; source pixels were not scaled)",
            args.width,
            args.height,
            args.anchor,
            args.fill,
            png.len()
        ),
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Output canvas width in pixels. Source pixels are not scaled." },
                    "height": { "type": "integer", "minimum": 1, "description": "Output canvas height in pixels. Source pixels are not scaled." },
                    "anchor": { "type": "string", "enum": ["center","top","bottom","left","right","top-left","top-right","bottom-left","bottom-right"], "default": "center", "description": "Where to pin the source image on the new canvas: controls padding when growing and what gets cropped when shrinking." },
                    "fill":   { "type": "string", "default": "#ffffff", "description": "Fill colour for newly exposed canvas: #rgb, #rgba, #rrggbb, #rrggbbaa, white, black, red, green, blue, gray, or transparent." }
                },
                "additionalProperties": false,
                "required": ["width", "height"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
