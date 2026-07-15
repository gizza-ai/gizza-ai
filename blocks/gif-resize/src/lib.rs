//! gizza-ai/gif-resize — resize an animated GIF to new dimensions, preserving
//! every frame, its timing, and the loop. Returns a GIF. Pure Rust (image
//! crate's GIF codec, no ffmpeg) → runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (GIF input + GIF bytes output → no page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_gif_resize_core::resize_gif;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    width: Option<u64>,
    #[serde(default)]
    height: Option<u64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("width")
                .min(1.0)
                .describe("Target width in pixels. Omit to compute from height, preserving aspect ratio."),
        )
        .param(
            Param::integer("height")
                .min(1.0)
                .describe("Target height in pixels. Omit to compute from width, preserving aspect ratio."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GifResize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resize an animated GIF to new dimensions",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Resize an animated GIF to new pixel dimensions, preserving every frame, its timing, and the loop. Provide width and/or height; if only one is given the other is computed to preserve the aspect ratio. Returns the resized GIF. Provide the GIF as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl GifResize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("gif-resize")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let w = args.width.map(|v| v.min(u32::MAX as u64) as u32);
    let h = args.height.map(|v| v.min(u32::MAX as u64) as u32);

    let res = resize_gif(&bytes, w, h).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &res.bytes,
        "image/gif",
        "resized.gif".to_string(),
        format!(
            "resized GIF: {}×{} → {}×{} ({} frames, {} bytes)",
            res.orig_width, res.orig_height, res.width, res.height, res.frames, res.bytes.len()
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Target width in pixels. Omit to compute from height, preserving aspect ratio." },
                    "height": { "type": "integer", "minimum": 1, "description": "Target height in pixels. Omit to compute from width, preserving aspect ratio." }
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
