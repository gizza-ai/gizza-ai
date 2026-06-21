//! gizza-ai/image-color-quantize — reduce an image to N colors with an optimal
//! palette (NeuQuant). Returns a PNG. Pure Rust (image + color_quant) — runs on
//! all backends incl. the chat SW. Surfaces: chat + CLI (image input + image
//! bytes output → no page, like normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_color_quantize_core::quantize;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_colors")]
    colors: u64,
}
fn default_colors() -> u64 {
    16
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::integer("colors")
            .min(2.0)
            .max(256.0)
            .default(16)
            .describe("Number of colors to reduce the image to, 2-256 (default 16). An optimal palette is derived from the image."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageColorQuantize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-color-quantize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce an image to N colors (quantize)",
    requires = ["wafer-run/network"],
    skill(
        description = "Reduce an image to at most N colors using NeuQuant quantization — an optimal palette is derived from the image itself (a posterized / retro look, and often a smaller file). colors is 2-256 (default 16). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageColorQuantize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-color-quantize")?;
    let colors = args.colors.clamp(2, 256) as usize;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = quantize(&bytes, colors).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "quantized.png".to_string(),
        format!("quantized to {colors} colors ({} bytes PNG)", png.len()),
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "colors": { "type": "integer", "minimum": 2, "maximum": 256, "default": 16, "description": "Number of colors to reduce the image to, 2-256 (default 16). An optimal palette is derived from the image." }
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
