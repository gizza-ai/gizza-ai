//! gizza-ai/animated-webp-to-gif — convert an animated (or still) WebP into an
//! animated GIF for maximum compatibility. Returns a GIF. Pure-Rust (image crate)
//! — runs on all backends incl. the chat SW. Surfaces: chat + CLI (image-bytes
//! input + image-bytes output → no page, like flip-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_animated_webp_to_gif_core::{webp_to_gif, LoopMode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 48 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_loop")]
    r#loop: String,
    #[serde(default)]
    speed: f64,
    #[serde(default)]
    delay: f64,
    #[serde(default)]
    width: f64,
}

fn default_loop() -> String {
    "infinite".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("loop", ["infinite", "once"])
                .default("infinite")
                .describe("GIF loop behaviour: infinite (loop forever, default) or once (play a single time)."),
        )
        .param(
            Param::integer("speed")
                .min(1.0)
                .max(30.0)
                .describe("Encoder quality/speed (1-30, default 10). Lower = better palette/quality but slower; higher = faster."),
        )
        .param(
            Param::integer("delay")
                .min(0.0)
                .max(60000.0)
                .describe("Override every frame's delay in milliseconds (default 0 = keep the WebP's original per-frame timing)."),
        )
        .param(
            Param::integer("width")
                .min(0.0)
                .max(4096.0)
                .describe("Output width in pixels; height scales to keep aspect ratio (default 0 = keep the source size)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AnimatedWebpToGif;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/animated-webp-to-gif",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an animated WebP into an animated GIF",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert an animated (or still) WebP into an animated GIF for maximum compatibility. Preserves every frame and its timing, with infinite looping by default. Provide the WebP as either url (HTTP/HTTPS) or ref from a prior tool call. Options: loop (infinite, default, or once), speed (1-30 encoder quality/speed, default 10; lower = better palette), delay (override every frame's delay in ms, default 0 = keep original timing), and width (resize output width in px, height scales to keep aspect ratio, default 0 = keep source size). Returns a GIF.",
        parameters = schema_json()
    ),
)]
impl AnimatedWebpToGif {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("animated-webp-to-gif")?;
    let loop_mode = LoopMode::parse(&args.r#loop).map_err(SkillError::InvalidArgs)?;
    let speed = if args.speed == 0.0 { 10 } else { args.speed.round() as i32 };
    let force_delay = if args.delay > 0.0 {
        Some(args.delay.round() as u32)
    } else {
        None
    };
    let width = if args.width > 0.0 {
        Some(args.width.round() as u32)
    } else {
        None
    };
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let gif =
        webp_to_gif(&bytes, loop_mode, speed, force_delay, width).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &gif,
        "image/gif",
        "converted.gif".to_string(),
        format!("converted animated WebP to GIF ({} bytes)", gif.len()),
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
                    "url":   { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":   { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "loop":  { "type": "string", "enum": ["infinite", "once"], "default": "infinite", "description": "GIF loop behaviour: infinite (loop forever, default) or once (play a single time)." },
                    "speed": { "type": "integer", "minimum": 1, "maximum": 30, "description": "Encoder quality/speed (1-30, default 10). Lower = better palette/quality but slower; higher = faster." },
                    "delay": { "type": "integer", "minimum": 0, "maximum": 60000, "description": "Override every frame's delay in milliseconds (default 0 = keep the WebP's original per-frame timing)." },
                    "width": { "type": "integer", "minimum": 0, "maximum": 4096, "description": "Output width in pixels; height scales to keep aspect ratio (default 0 = keep the source size)." }
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
