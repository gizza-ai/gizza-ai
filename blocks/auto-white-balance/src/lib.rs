//! gizza-ai/auto-white-balance — remove common image color casts using
//! gray-world or white-patch channel balancing. Returns a PNG. Pure Rust
//! (`image` crate), so it runs on all backends including the chat service worker.
//! Image-bytes output tools expose chat + CLI surfaces; there is no standalone
//! page for browser recompute.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_auto_white_balance_core::white_balance;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_strength")]
    strength: f64,
}

fn default_method() -> String { "gray-world".to_string() }
fn default_strength() -> f64 { 100.0 }

/// Single source for the chat schema (and CLI). The image source is either an
/// HTTP(S) URL or a prior tool ref. `method` selects the estimation heuristic;
/// `strength` blends the correction back toward the original image.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("method", ["gray-world", "white-patch"])
                .default("gray-world")
                .describe("White-balance estimate to use: gray-world neutralizes average color; white-patch maps channel maxima toward white."),
        )
        .param(
            Param::number("strength")
                .min(0.0)
                .max(100.0)
                .default(100.0)
                .describe("Correction strength as a percentage, 0 to 100 (0 = unchanged, 100 = full correction)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct AutoWhiteBalance;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/auto-white-balance",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto white-balance an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Remove common image color casts by white-balancing an image with either gray-world (neutralize average color) or white-patch (map brightest channel values toward white). strength is 0..100 percent and blends the correction with the original. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl AutoWhiteBalance {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("auto-white-balance")?;
    let (bytes, _mime, _name) = resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = white_balance(&bytes, &args.method, args.strength).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "white-balanced.png".to_string(),
        format!(
            "auto white-balanced with {} at {}% strength — {} bytes PNG",
            args.method, args.strength, png.len()
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
                    "url":      { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "method":   { "type": "string", "enum": ["gray-world", "white-patch"], "default": "gray-world", "description": "White-balance estimate to use: gray-world neutralizes average color; white-patch maps channel maxima toward white." },
                    "strength": { "type": "number", "minimum": 0, "maximum": 100, "default": 100.0, "description": "Correction strength as a percentage, 0 to 100 (0 = unchanged, 100 = full correction)." }
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
