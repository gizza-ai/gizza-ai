//! gizza-ai/image-cover-fit — scale-and-centre-crop ("cover") an image so it
//! completely fills target dimensions while preserving aspect ratio, cropping
//! the overflow around a chosen anchor. Returns a PNG of exactly the requested
//! size. Pure-Rust (image crate) — runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (image input + image-bytes output → no page, like
//! image-contain-fit / flip-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_cover_fit_core::{cover_fit, Anchor};
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
    #[serde(default = "default_allow_upscale")]
    allow_upscale: bool,
}

fn default_anchor() -> String {
    "center".to_string()
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
                .describe("Target width in pixels (the output is exactly this wide)."),
        )
        .param(
            Param::integer("height")
                .required()
                .min(1.0)
                .describe("Target height in pixels (the output is exactly this tall)."),
        )
        .param(
            Param::enumv(
                "anchor",
                [
                    "center",
                    "top",
                    "bottom",
                    "left",
                    "right",
                    "top-left",
                    "top-right",
                    "bottom-left",
                    "bottom-right",
                ],
            )
            .default("center")
            .describe("Which part of the image to keep when cropping the overflow (gravity)."),
        )
        .param(
            Param::boolean("allow_upscale")
                .default(true)
                .describe("If true (default), upscale images smaller than the target so they fully cover it; if false, keep small images at original size (the output may be smaller than the target rather than padded)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageCoverFit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-cover-fit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scale and centre-crop an image to completely fill target dimensions while preserving aspect ratio (cover)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scale and centre-crop ('cover') an image so it completely fills a target width x height while preserving its aspect ratio. The image is scaled to the smallest size that covers the box, then the overflow is cropped away around the chosen anchor (gravity). anchor is one of center, top, bottom, left, right, top-left, top-right, bottom-left, bottom-right (default center); set allow_upscale=false to keep images smaller than the target at original size. The result is exactly the requested size with no padding (use image-contain-fit for letterboxing instead). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageCoverFit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-cover-fit")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid image-cover-fit args: width and height must be > 0".into(),
        ));
    }
    let anchor = Anchor::parse(&args.anchor).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = cover_fit(&bytes, args.width, args.height, anchor, args.allow_upscale)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "cover-fit.png".to_string(),
        format!(
            "cover-fit image into {}x{} (anchor {}, {} bytes PNG)",
            args.width,
            args.height,
            args.anchor,
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
                    "width":         { "type": "integer", "minimum": 1, "description": "Target width in pixels (the output is exactly this wide)." },
                    "height":        { "type": "integer", "minimum": 1, "description": "Target height in pixels (the output is exactly this tall)." },
                    "anchor":        { "type": "string", "enum": ["center","top","bottom","left","right","top-left","top-right","bottom-left","bottom-right"], "default": "center", "description": "Which part of the image to keep when cropping the overflow (gravity)." },
                    "allow_upscale": { "type": "boolean", "default": true, "description": "If true (default), upscale images smaller than the target so they fully cover it; if false, keep small images at original size (the output may be smaller than the target rather than padded)." }
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
