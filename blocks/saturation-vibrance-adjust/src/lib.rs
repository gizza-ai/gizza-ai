//! gizza-ai/saturation-vibrance-adjust — selective saturation for an image.
//! `vibrance` boosts muted colours hard while sparing already-vivid pixels and
//! (optionally) skin tones; `saturation` is the classic flat scale. Returns a
//! PNG. Pure Rust (image crate) — runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! image-hsl-adjust / normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_saturation_vibrance_adjust_core::adjust;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    vibrance: f64,
    #[serde(default)]
    saturation: f64,
    #[serde(default = "yes")]
    protect_skin: bool,
}
fn yes() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("vibrance")
                .min(-1.0)
                .max(1.0)
                .default(0.0)
                .describe(
                    "Selective saturation, -1 to 1 (0 = no change). Positive boosts muted \
                     colours hard while barely touching already-vivid pixels; negative mutes.",
                ),
        )
        .param(
            Param::number("saturation")
                .min(-1.0)
                .max(1.0)
                .default(0.0)
                .describe(
                    "Flat global saturation, -1 to 1 (0 = no change, -1 = grayscale, 1 = 2×). \
                     Applied uniformly to every pixel, on top of vibrance.",
                ),
        )
        .param(
            Param::boolean("protect_skin")
                .default(true)
                .describe("Spare skin-tone hues from the vibrance boost so faces stay natural. Default on."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SaturationVibranceAdjust;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/saturation-vibrance-adjust",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Selective vibrance + saturation for an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Boost or mute an image's colour intensity. vibrance (-1..1, 0 = no change) is a SELECTIVE, nonlinear push: positive lifts muted colours strongly while leaving already-vivid pixels alone, and by default protects skin tones so faces don't turn orange. saturation (-1..1, 0 = no change, -1 = grayscale, 1 = double) is the classic flat scale applied to every pixel. Set protect_skin=false to boost skin too. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl SaturationVibranceAdjust {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("saturation-vibrance-adjust")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = adjust(&bytes, args.vibrance as f32, args.saturation as f32, args.protect_skin)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "vibrance-adjusted.png".to_string(),
        format!(
            "Vibrance adjusted (vibrance {:+}, saturation {:+}, protect_skin {}) — {} bytes PNG",
            args.vibrance, args.saturation, args.protect_skin, png.len()
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
                    "url":          { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "vibrance":     { "type": "number", "minimum": -1, "maximum": 1, "default": 0.0, "description": "Selective saturation, -1 to 1 (0 = no change). Positive boosts muted colours hard while barely touching already-vivid pixels; negative mutes." },
                    "saturation":   { "type": "number", "minimum": -1, "maximum": 1, "default": 0.0, "description": "Flat global saturation, -1 to 1 (0 = no change, -1 = grayscale, 1 = 2×). Applied uniformly to every pixel, on top of vibrance." },
                    "protect_skin": { "type": "boolean", "default": true, "description": "Spare skin-tone hues from the vibrance boost so faces stay natural. Default on." }
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
