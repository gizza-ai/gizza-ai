//! gizza-ai/image-opacity — set or scale the alpha/opacity channel of an image,
//! producing a semi-transparent PNG. Pure Rust (image crate) — runs on all
//! backends incl. the chat SW. Surfaces: chat + CLI (image input + image bytes
//! output → no page, like normalize-image / image-hsl-adjust).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_opacity_core::{apply, Mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "half")]
    opacity: f64,
    #[serde(default = "default_mode")]
    mode: String,
}
fn half() -> f64 {
    0.5
}
fn default_mode() -> String {
    "scale".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("opacity")
                .min(0.0)
                .max(1.0)
                .default(0.5)
                .describe("Target opacity from 0.0 (fully transparent) to 1.0 (fully opaque)."),
        )
        .param(
            Param::enumv("mode", ["scale", "set"])
                .default("scale")
                .describe("'scale' multiplies each pixel's existing alpha by opacity (keeps existing transparency); 'set' overwrites every pixel's alpha with a uniform opacity."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageOpacity;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-opacity",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Set or scale an image's opacity (alpha)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Set or scale the alpha/opacity of an image, producing a semi-transparent PNG. opacity is 0.0 (fully transparent) to 1.0 (fully opaque). mode='scale' multiplies each pixel's existing alpha by opacity (preserving existing transparency); mode='set' overwrites every pixel's alpha with a uniform opacity. Always returns a PNG so the alpha channel is kept. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageOpacity {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-opacity")?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = apply(&bytes, args.opacity as f32, mode).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "opacity.png".to_string(),
        format!(
            "Opacity {} ({} mode) — {} bytes PNG",
            args.opacity, args.mode, png.len()
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
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "opacity": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5, "description": "Target opacity from 0.0 (fully transparent) to 1.0 (fully opaque)." },
                    "mode":    { "type": "string", "enum": ["scale", "set"], "default": "scale", "description": "'scale' multiplies each pixel's existing alpha by opacity (keeps existing transparency); 'set' overwrites every pixel's alpha with a uniform opacity." }
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
