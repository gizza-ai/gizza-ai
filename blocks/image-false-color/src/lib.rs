//! gizza-ai/image-false-color — apply a scientific colormap to an image's luminance.
//! Returns a PNG. Pure-Rust (image crate). Surfaces: chat + CLI (image input +
//! image bytes output → no page, like colorblind-simulator / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_false_color_core::{false_color, Colormap};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_colormap")]
    colormap: String,
    #[serde(default)]
    invert: bool,
}
fn default_colormap() -> String {
    "viridis".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv(
                "colormap",
                ["viridis", "magma", "inferno", "plasma", "cividis", "turbo", "jet", "hot", "grayscale"],
            )
            .default("viridis")
            .describe("Scientific colormap to map luminance through: viridis (default), magma, inferno, plasma, cividis (colour-blind-friendly), turbo, jet, hot (thermal black→red→yellow→white), or grayscale."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("When true, flip the luminance scale (1 - L) before lookup so dark areas map to the high end of the colormap. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageFalseColor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-false-color",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a scientific colormap to an image's luminance.",
    requires = ["wafer-run/network"],
    skill(
        description = "Map a grayscale, thermal, depth, or scientific image's per-pixel luminance through a scientific colormap to produce a false-colour heatmap (PNG). colormap = viridis (default), magma, inferno, plasma, cividis, turbo, jet, hot, or grayscale; set invert = true to flip the scale so dark areas become the high end. Alpha is preserved. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ImageFalseColor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-false-color")?;
    let cmap = Colormap::parse(&args.colormap).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = false_color(&bytes, cmap, args.invert).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        format!("false-color-{}.png", args.colormap),
        format!("false-colour {} ({} bytes PNG)", args.colormap, png.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "colormap": { "type": "string", "enum": ["viridis", "magma", "inferno", "plasma", "cividis", "turbo", "jet", "hot", "grayscale"], "default": "viridis", "description": "Scientific colormap to map luminance through: viridis (default), magma, inferno, plasma, cividis (colour-blind-friendly), turbo, jet, hot (thermal black→red→yellow→white), or grayscale." },
                    "invert":   { "type": "boolean", "default": false, "description": "When true, flip the luminance scale (1 - L) before lookup so dark areas map to the high end of the colormap. Default false." }
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
