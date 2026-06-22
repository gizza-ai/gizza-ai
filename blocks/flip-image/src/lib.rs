//! gizza-ai/flip-image — flip an image horizontally (mirror) or vertically.
//! Returns a PNG. Pure-Rust (image crate) — runs on all backends incl. the chat
//! SW. Surfaces: chat + CLI (image input + image bytes output → no page, like
//! normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_flip_image_core::{flip, Direction};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_direction")]
    direction: String,
}

fn default_direction() -> String {
    "horizontal".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::enumv("direction", ["horizontal", "vertical"])
            .default("horizontal")
            .describe("Flip axis: horizontal (left-right mirror) or vertical (top-bottom)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FlipImage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/flip-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flip an image horizontally or vertically",
    requires = ["wafer-run/network"],
    skill(
        description = "Flip an image horizontally (left-right mirror, direction=horizontal, default) or vertically (top-bottom, direction=vertical). Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl FlipImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("flip-image")?;
    let dir = Direction::parse(&args.direction).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = flip(&bytes, dir).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "flipped.png".to_string(),
        format!("flipped image ({}, {} bytes PNG)", args.direction, png.len()),
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "direction": { "type": "string", "enum": ["horizontal", "vertical"], "default": "horizontal", "description": "Flip axis: horizontal (left-right mirror) or vertical (top-bottom)." }
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
