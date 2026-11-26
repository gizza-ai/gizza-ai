//! gizza-ai/image-rounded-corners — round the corners of an image at a chosen
//! radius and return a PNG. The image keeps its ORIGINAL dimensions/aspect ratio
//! (corners are masked, the image is NOT cropped to a square — distinct from
//! `image-round-avatar`). Radius can be pixels or a percent of the shorter side
//! (50% fully rounds it into a pill/circle); corners can be rounded selectively;
//! masked corners are transparent by default or filled with a solid background.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::round_corners` (alpha
//! mask via the pure-Rust `image` crate) → PNG media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI (this handler) and the standalone page (web/, `wasm-image`
//! runtime) — both call core `apply` with the same `Params`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_rounded_corners_core::{apply, Params};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(flatten)]
    params: Params,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::number("radius").min(0.0).describe("Corner radius. In px by default, or a percent of the shorter side when unit=percent (percent 50 fully rounds the short side into a pill/circle). Default: 12% of the shorter side."))
        .param(Param::enumv("unit", ["px", "percent"]).default("px").describe("How radius is measured: px (absolute pixels, default) or percent (0-50, relative to the shorter side; 50 = fully rounded)."))
        .param(Param::enumv("corners", ["all", "top", "bottom", "left", "right"]).default("all").describe("Which corners to round: all (default), top, bottom, left, or right."))
        .param(Param::string("background").default("transparent").describe("Fill for the masked corners: transparent (default) keeps them see-through, or a solid color (hex like #3366ff or a common name like white) fills them and returns a fully opaque PNG."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageRoundedCorners;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-rounded-corners",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Round the corners of an image at a chosen radius, returned as a PNG",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Round the corners of an image at a chosen radius and return a transparent PNG. The image keeps its original dimensions and aspect ratio (corners are masked, NOT cropped to a square). radius is in pixels by default, or a percent of the shorter side when unit=percent (percent 50 fully rounds the short side into a pill/circle); default is 12% of the shorter side. corners selects which corners to round: all (default), top, bottom, left, or right. background fills the masked corners with a solid color (hex like #3366ff or a common name) and returns a fully opaque PNG; the default 'transparent' keeps the corners see-through. Edges are anti-aliased. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageRoundedCorners {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-rounded-corners")?;
    // Fail fast on bad options before fetching the image.
    args.params.options().map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let (png, out_name) = apply(&bytes, &args.params).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &png,
        "image/png",
        out_name,
        format!("rounded the {} corners ({} bytes PNG)", args.params.corners, png.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_flatten_params_from_chat_json() {
        let args: Args =
            serde_json::from_str(r#"{"url": "https://example.com/a.png", "radius": 24, "corners": "top"}"#).unwrap();
        assert_eq!(
            args.params,
            Params { radius: Some(24.0), corners: "top".into(), ..Params::default() }
        );
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "radius":     { "type": "number", "minimum": 0, "description": "Corner radius. In px by default, or a percent of the shorter side when unit=percent (percent 50 fully rounds the short side into a pill/circle). Default: 12% of the shorter side." },
                    "unit":       { "type": "string", "enum": ["px", "percent"], "default": "px", "description": "How radius is measured: px (absolute pixels, default) or percent (0-50, relative to the shorter side; 50 = fully rounded)." },
                    "corners":    { "type": "string", "enum": ["all", "top", "bottom", "left", "right"], "default": "all", "description": "Which corners to round: all (default), top, bottom, left, or right." },
                    "background": { "type": "string", "default": "transparent", "description": "Fill for the masked corners: transparent (default) keeps them see-through, or a solid color (hex like #3366ff or a common name like white) fills them and returns a fully opaque PNG." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
