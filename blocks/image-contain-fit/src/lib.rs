//! gizza-ai/image-contain-fit — fit (letterbox) an image inside target
//! dimensions preserving aspect ratio, padding the leftover space with a chosen
//! colour. Returns a PNG of exactly the requested size. Pure-Rust (image crate)
//! — runs on all backends incl. the chat SW. Surfaces: chat + CLI (image input
//! + image-bytes output → no page, like flip-image / normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_contain_fit_core::{contain_fit, parse_color};
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
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_allow_upscale")]
    allow_upscale: bool,
}

fn default_background() -> String {
    "#ffffff".to_string()
}

fn default_allow_upscale() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("width")
                .required()
                .min(1.0)
                .describe("Target canvas width in pixels."),
        )
        .param(
            Param::integer("height")
                .required()
                .min(1.0)
                .describe("Target canvas height in pixels."),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe("Pad colour for the letterbox bars: a hex value like #ffffff or a name (white, black, transparent)."),
        )
        .param(
            Param::boolean("allow_upscale")
                .default(true)
                .describe("If true (default), upscale images smaller than the target to fill the box; if false, keep small images at original size centred within the padding."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageContainFit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-contain-fit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fit an image inside target dimensions preserving aspect ratio, padding with a chosen colour (letterbox)",
    requires = ["wafer-run/network"],
    skill(
        description = "Fit (letterbox) an image inside a target width x height preserving its aspect ratio, padding the leftover space with a chosen background colour so the result is exactly the requested size. background accepts a hex value like #ffffff or a name (white, black, transparent); set allow_upscale=false to keep images smaller than the target at original size. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageContainFit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-contain-fit")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid image-contain-fit args: width and height must be > 0".into(),
        ));
    }
    let pad = parse_color(&args.background).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = contain_fit(&bytes, args.width, args.height, pad, args.allow_upscale)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "contain-fit.png".to_string(),
        format!(
            "fit image into {}x{} (background {}, {} bytes PNG)",
            args.width,
            args.height,
            args.background,
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
                    "url":           { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":           { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":         { "type": "integer", "minimum": 1, "description": "Target canvas width in pixels." },
                    "height":        { "type": "integer", "minimum": 1, "description": "Target canvas height in pixels." },
                    "background":    { "type": "string", "default": "#ffffff", "description": "Pad colour for the letterbox bars: a hex value like #ffffff or a name (white, black, transparent)." },
                    "allow_upscale": { "type": "boolean", "default": true, "description": "If true (default), upscale images smaller than the target to fill the box; if false, keep small images at original size centred within the padding." }
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
