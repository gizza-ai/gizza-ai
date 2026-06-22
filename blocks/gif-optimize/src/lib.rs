//! gizza-ai/gif-optimize — shrink an animated GIF by scaling it down, dropping
//! frames, and/or posterizing colors. Returns a GIF. Pure Rust (image crate's
//! GIF codec, no ffmpeg) → runs on all backends incl. the chat SW. Surfaces:
//! chat + CLI (GIF input + GIF bytes output → no page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_gif_optimize_core::optimize;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_frame_step")]
    frame_step: u64,
    #[serde(default = "default_colors")]
    color_bits: u64,
}
fn default_scale() -> f64 {
    1.0
}
fn default_frame_step() -> u64 {
    1
}
fn default_colors() -> u64 {
    8
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("scale")
                .min(0.01)
                .max(1.0)
                .describe("Resize factor for every frame, 0.01-1.0 (default 1.0 = no resize). 0.5 halves width & height."),
        )
        .param(
            Param::integer("frame_step")
                .min(1.0)
                .describe("Keep every Nth frame to thin the animation (default 1 = keep all). Dropped frames' delays roll into the kept frame so timing is preserved."),
        )
        .param(
            Param::integer("color_bits")
                .min(1.0)
                .max(8.0)
                .describe("Bits per color channel for lossy color reduction, 1-8 (default 8 = no reduction). Lower = fewer colors and a smaller file."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GifOptimize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-optimize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shrink an animated GIF (scale, frames, colors)",
    requires = ["wafer-run/network"],
    skill(
        description = "Shrink an animated GIF. scale (0.01-1.0, default 1.0) resizes every frame; frame_step (default 1) keeps every Nth frame (timing preserved by rolling dropped delays forward); color_bits (1-8, default 8) does lossy per-channel color reduction. Returns the optimized GIF. Provide the GIF as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl GifOptimize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("gif-optimize")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let input_len = bytes.len();
    let step = args.frame_step.max(1) as usize;
    let bits = args.color_bits.clamp(1, 8) as u8;

    let res = optimize(&bytes, args.scale, step, bits).map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &res.bytes,
        "image/gif",
        "optimized.gif".to_string(),
        format!(
            "optimized GIF: {}→{} frames, {}×{}, {} → {} bytes",
            res.frames_in, res.frames_out, res.width, res.height, input_len, res.bytes.len()
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
                    "url":        { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "scale":      { "type": "number", "minimum": 0.01, "maximum": 1, "description": "Resize factor for every frame, 0.01-1.0 (default 1.0 = no resize). 0.5 halves width & height." },
                    "frame_step": { "type": "integer", "minimum": 1, "description": "Keep every Nth frame to thin the animation (default 1 = keep all). Dropped frames' delays roll into the kept frame so timing is preserved." },
                    "color_bits": { "type": "integer", "minimum": 1, "maximum": 8, "description": "Bits per color channel for lossy color reduction, 1-8 (default 8 = no reduction). Lower = fewer colors and a smaller file." }
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
