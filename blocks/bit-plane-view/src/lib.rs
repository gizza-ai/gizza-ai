//! gizza-ai/bit-plane-view — extract and render a single bit plane of a chosen
//! colour channel from an image, to reveal bit-level patterns and hidden data
//! (steganography / image forensics). Pure-Rust (image crate).
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! color-channel-split / image-false-color / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_bit_plane_view_core::{bit_plane, Channel, Mode};
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
    #[serde(default = "default_bit")]
    bit: u64,
    #[serde(default = "default_mode")]
    mode: String,
}
fn default_channel() -> String {
    "red".to_string()
}
fn default_bit() -> u64 {
    0
}
fn default_mode() -> String {
    "binary".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("channel", ["red", "green", "blue", "alpha", "gray"])
                .default("red")
                .describe(
                    "Which channel's bit plane to extract: red (default), green, blue, alpha, or gray (Rec. 601 luminance of the RGB pixel).",
                ),
        )
        .param(
            Param::integer("bit")
                .min(0.0)
                .max(7.0)
                .default(0)
                .describe(
                    "Which bit plane to isolate, 0-7. 0 is the least-significant bit (LSB, default) where steganographic payloads usually hide; 7 is the most-significant bit (MSB), which resembles the visible image.",
                ),
        )
        .param(
            Param::enumv("mode", ["binary", "weighted", "color"])
                .default("binary")
                .describe(
                    "How to render the plane: binary (default) shows a set bit as white and a clear bit as black — the maximum-contrast stego / forensics view; weighted renders the bit at its positional weight (bit << plane) as a gray level, showing its real contribution to the pixel; color shows a set bit in the channel's colour (red→red, gray/alpha→white) over black.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BitPlaneView;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bit-plane-view",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a single bit plane of an image channel as a PNG.",
    requires = ["wafer-run/network"],
    skill(
        description = "Isolate one bit plane of an image channel to reveal bit-level patterns and hidden data (steganography / image forensics). channel = red (default), green, blue, alpha, or gray (Rec. 601 luminance). bit = 0-7, where 0 is the least-significant bit (LSB, default) — the classic place steganographic payloads hide, which pops out as structured noise against the natural image's randomness — and 7 is the most-significant bit (MSB). mode = binary (default) renders a set bit white and a clear bit black (max contrast); weighted renders the bit at its positional weight (bit << plane) as a gray level; color renders a set bit in the channel's colour over black. Output is a PNG the same size as the input. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl BitPlaneView {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("bit-plane-view")?;
    let channel = Channel::parse(&args.channel).map_err(SkillError::InvalidArgs)?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let bit = args.bit.min(u32::MAX as u64) as u32;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = bit_plane(&bytes, channel, bit, mode).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        format!("{}-bit{}.png", args.channel, args.bit),
        format!(
            "{} channel bit {} ({} mode, {} bytes PNG)",
            args.channel,
            args.bit,
            args.mode,
            png.len()
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
                    "channel": { "type": "string", "enum": ["red", "green", "blue", "alpha", "gray"], "default": "red", "description": "Which channel's bit plane to extract: red (default), green, blue, alpha, or gray (Rec. 601 luminance of the RGB pixel)." },
                    "bit":     { "type": "integer", "minimum": 0, "maximum": 7, "default": 0, "description": "Which bit plane to isolate, 0-7. 0 is the least-significant bit (LSB, default) where steganographic payloads usually hide; 7 is the most-significant bit (MSB), which resembles the visible image." },
                    "mode":    { "type": "string", "enum": ["binary", "weighted", "color"], "default": "binary", "description": "How to render the plane: binary (default) shows a set bit as white and a clear bit as black — the maximum-contrast stego / forensics view; weighted renders the bit at its positional weight (bit << plane) as a gray level, showing its real contribution to the pixel; color shows a set bit in the channel's colour (red→red, gray/alpha→white) over black." }
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
