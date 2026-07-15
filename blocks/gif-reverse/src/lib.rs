//! gizza-ai/gif-reverse — reverse the playback order of an animated GIF. Returns
//! a GIF. Pure Rust (image crate's GIF codec, no ffmpeg) → runs on all backends
//! including the chat SW. Surfaces: chat + CLI (GIF input + GIF bytes output →
//! no page). Optional boomerang (ping-pong) mode plays forward then backward.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_gif_reverse_core::reverse;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    boomerang: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::boolean("boomerang")
            .describe("Play forward then backward (ping-pong) for a seamless loop instead of a plain reverse (default false). Output has 2N-1 frames."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GifReverse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-reverse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reverse an animated GIF's frame order",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Reverse the playback order of an animated GIF (last frame first). Each frame keeps its own delay so the speed is unchanged, and the result loops forever. Set boomerang=true to instead play forward then backward (ping-pong) for a seamless loop. Returns the new GIF. Provide the GIF as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl GifReverse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("gif-reverse")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let input_len = bytes.len();

    let res = reverse(&bytes, args.boomerang).map_err(SkillError::InvalidArgs)?;

    let mode = if args.boomerang { "boomerang" } else { "reversed" };
    build_media_envelope(
        &res.bytes,
        "image/gif",
        "reversed.gif".to_string(),
        format!(
            "{} GIF: {}→{} frames, {}×{}, {} → {} bytes",
            mode, res.frames_in, res.frames_out, res.width, res.height, input_len, res.bytes.len()
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
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "boomerang": { "type": "boolean", "description": "Play forward then backward (ping-pong) for a seamless loop instead of a plain reverse (default false). Output has 2N-1 frames." }
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
