//! gizza-ai/color-channel-split — extract a single colour channel (red, green,
//! blue, or alpha) from an image as its own PNG. Pure-Rust (image crate).
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! image-false-color / colorblind-simulator / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_color_channel_split_core::{channel_split, Channel, Mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_channel")]
    channel: String,
    #[serde(default = "default_mode")]
    mode: String,
}
fn default_channel() -> String {
    "red".to_string()
}
fn default_mode() -> String {
    "grayscale".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv(
                "channel",
                ["red", "green", "blue", "alpha", "cyan", "magenta", "yellow", "key"],
            )
            .default("red")
            .describe(
                "Which channel to extract: the RGBA channels red (default), green, blue, alpha; or a CMYK separation cyan, magenta, yellow, key (black). CMYK is derived from RGB.",
            ),
        )
        .param(
            Param::enumv("mode", ["grayscale", "color"])
                .default("grayscale")
                .describe("How to render the channel: grayscale (default) shows the channel value as a gray level (R=G=B=value), the classic stego / forensics view; color keeps the value in its own colour slot with the others zeroed (red→red-only, etc.; alpha→opacity over black)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorChannelSplit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-channel-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a single colour channel (R/G/B/A) from an image as a PNG.",
    requires = ["wafer-run/network"],
    skill(
        description = "Split an image into a single colour channel for steganography and colour analysis. channel = red (default), green, blue, alpha, or a CMYK separation cyan, magenta, yellow, key (black, derived from RGB). mode = grayscale (default) renders the channel value as a gray level (R=G=B=value) — the classic per-channel stego / forensics view; mode = color keeps the value in its own colour slot with the others zeroed (red→red-only image, alpha→opacity over black, CMYK→its ink colour). Output is a PNG the same size as the input. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl ColorChannelSplit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("color-channel-split")?;
    let channel = Channel::parse(&args.channel).map_err(SkillError::InvalidArgs)?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = channel_split(&bytes, channel, mode).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        format!("{}-channel.png", args.channel),
        format!("{} channel ({} mode, {} bytes PNG)", args.channel, args.mode, png.len()),
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
                    "channel": { "type": "string", "enum": ["red", "green", "blue", "alpha", "cyan", "magenta", "yellow", "key"], "default": "red", "description": "Which channel to extract: the RGBA channels red (default), green, blue, alpha; or a CMYK separation cyan, magenta, yellow, key (black). CMYK is derived from RGB." },
                    "mode":    { "type": "string", "enum": ["grayscale", "color"], "default": "grayscale", "description": "How to render the channel: grayscale (default) shows the channel value as a gray level (R=G=B=value), the classic stego / forensics view; color keeps the value in its own colour slot with the others zeroed (red→red-only, etc.; alpha→opacity over black)." }
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
