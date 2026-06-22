//! gizza-ai/lsb-extract — recover a hidden payload from an image's pixel LSBs
//! (least-significant-bit steganography). The inverse of `lsb-embed`.
//!
//! Pipeline: resolve the carrier image (URL fetch or attachment ref) →
//! `core::extract_auto` (find a `gizza-ai/lsb-embed` LSB1 header, RGB, depth
//! 1..=4) or `core::extract_raw` (read the low N bits of selectable channels in
//! pixel order, no header) → flat JSON the LLM reads directly: the recovered
//! text (if valid UTF-8), a hex preview of the bytes, the byte length, the depth
//! used, and the channels read.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text report — the F3 no-page
//! file-input pattern, like strings / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_lsb_extract_core::{extract_auto, extract_raw, sniff_filetype, BitOrder, Channels};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;
/// Cap on the recovered payload returned to the LLM/CLI in raw mode (raw mode
/// has no length header, so it dumps the whole bit-plane capacity otherwise).
const MAX_PAYLOAD: usize = 256 * 1024;
/// How many bytes of the payload to show as a hex preview.
const HEX_PREVIEW: usize = 256;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_channels")]
    channels: String,
    #[serde(default = "default_bits")]
    bits_per_channel: i64,
    #[serde(default = "default_bit_order")]
    bit_order: String,
    #[serde(default)]
    invert: bool,
}
fn default_mode() -> String {
    "auto".to_string()
}
fn default_channels() -> String {
    "rgb".to_string()
}
fn default_bits() -> i64 {
    1
}
fn default_bit_order() -> String {
    "msb".to_string()
}

#[derive(Serialize)]
struct Resp {
    /// Recovered payload as UTF-8 text, when it is valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Hex preview of the recovered bytes (first 256 bytes).
    hex_preview: String,
    /// Total recovered byte length.
    byte_length: usize,
    /// `true` when the payload was valid UTF-8 (so `text` is present).
    is_text: bool,
    /// bits/channel actually used.
    bits_per_channel: u8,
    /// Channels read, e.g. "rgb".
    channels: String,
    /// Extraction mode used: "auto" or "raw".
    mode: String,
    /// In auto mode, whether a valid lsb-embed (LSB1) header was found.
    header_found: bool,
    /// Best-effort file-type sniff of the recovered bytes when not plain text
    /// (e.g. "PNG image", "ZIP archive").
    #[serde(skip_serializing_if = "Option::is_none")]
    detected_filetype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// `true` when the payload was truncated to the response cap (raw mode).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["auto", "raw"]).default("auto").describe(
                "auto (default): find a gizza-ai/lsb-embed payload (LSB1 header, RGB, depth auto-detected 1-4). raw: read the low bits of the chosen channels in pixel order with no header.",
            ),
        )
        .param(
            Param::string("channels")
                .default("rgb")
                .describe("Raw mode only: which colour channels to read, any of r,g,b,a (e.g. \"rgb\", \"r\", \"rgba\"). Default \"rgb\". Ignored in auto mode."),
        )
        .param(
            Param::integer("bits_per_channel")
                .default(1)
                .min(1.0)
                .max(8.0)
                .describe("Raw mode only: low bits to read per channel, 1-8 (default 1). Must match how the payload was embedded. Ignored in auto mode."),
        )
        .param(
            Param::enumv("bit_order", ["msb", "lsb"]).default("msb").describe(
                "Raw mode only: bit order within each channel field — msb (default, like lsb-embed) or lsb (common in other encoders). Ignored in auto mode.",
            ),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("Raw mode only: bitwise-invert (XOR 0xFF) the recovered bytes, to undo a common obfuscation. Ignored in auto mode."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LsbExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lsb-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a hidden message from an image (LSB steganography)",
    requires = ["wafer-run/network"],
    skill(
        description = "Recover a hidden payload from an image's least-significant bits (LSB steganography) — the inverse of lsb-embed. mode=auto (default) finds a gizza-ai/lsb-embed payload (LSB1 header, RGB channels, bit depth auto-detected 1-4) and returns the hidden text. mode=raw reads the low bits-per-channel bits of the chosen channels (channels: any of r,g,b,a; bits_per_channel 1-8; bit_order msb or lsb; optional invert to XOR 0xFF) in pixel order with no header — use it for payloads embedded by other LSB tools. Returns the recovered text (if valid UTF-8), a hex preview, byte length, a best-effort file-type sniff of binary payloads, and the depth + channels used. The carrier MUST be lossless (PNG/BMP/etc.); a JPEG re-encode destroys the hidden bits. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior image tool call). Runs locally — the image never leaves the device.",
        parameters = schema_json()
    ),
)]
impl LsbExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn hex_preview(bytes: &[u8]) -> String {
    let n = bytes.len().min(HEX_PREVIEW);
    let mut s = String::with_capacity(n * 2);
    for b in &bytes[..n] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("lsb-extract")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let mode = args.mode.trim().to_lowercase();
    let mut extracted = match mode.as_str() {
        "auto" => extract_auto(&bytes).map_err(SkillError::InvalidArgs)?,
        "raw" => {
            if !(1..=8).contains(&args.bits_per_channel) {
                return Err(SkillError::InvalidArgs(
                    "bits_per_channel must be between 1 and 8".into(),
                ));
            }
            let chans = Channels::parse(&args.channels).map_err(SkillError::InvalidArgs)?;
            let order = BitOrder::parse(&args.bit_order).map_err(SkillError::InvalidArgs)?;
            extract_raw(
                &bytes,
                chans,
                args.bits_per_channel as u8,
                order,
                args.invert,
                MAX_PAYLOAD,
            )
            .map_err(SkillError::InvalidArgs)?
        }
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "mode must be \"auto\" or \"raw\", got \"{other}\""
            )))
        }
    };

    let truncated = extracted.bytes.len() >= MAX_PAYLOAD;
    if extracted.bytes.len() > MAX_PAYLOAD {
        extracted.bytes.truncate(MAX_PAYLOAD);
        extracted.text = String::from_utf8(extracted.bytes.clone()).ok();
    }

    let detected_filetype = if extracted.text.is_none() {
        sniff_filetype(&extracted.bytes).map(|s| s.to_string())
    } else {
        None
    };
    let resp = Resp {
        is_text: extracted.text.is_some(),
        text: extracted.text,
        hex_preview: hex_preview(&extracted.bytes),
        byte_length: extracted.bytes.len(),
        bits_per_channel: extracted.bits_per_channel,
        channels: extracted.channels,
        mode,
        header_found: extracted.header_found,
        detected_filetype,
        filename: (!filename.is_empty()).then_some(filename),
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize lsb-extract response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":              { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":              { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":             { "type": "string", "enum": ["auto", "raw"], "default": "auto", "description": "auto (default): find a gizza-ai/lsb-embed payload (LSB1 header, RGB, depth auto-detected 1-4). raw: read the low bits of the chosen channels in pixel order with no header." },
                    "channels":         { "type": "string", "default": "rgb", "description": "Raw mode only: which colour channels to read, any of r,g,b,a (e.g. \"rgb\", \"r\", \"rgba\"). Default \"rgb\". Ignored in auto mode." },
                    "bits_per_channel": { "type": "integer", "minimum": 1, "maximum": 8, "default": 1, "description": "Raw mode only: low bits to read per channel, 1-8 (default 1). Must match how the payload was embedded. Ignored in auto mode." },
                    "bit_order":        { "type": "string", "enum": ["msb", "lsb"], "default": "msb", "description": "Raw mode only: bit order within each channel field — msb (default, like lsb-embed) or lsb (common in other encoders). Ignored in auto mode." },
                    "invert":           { "type": "boolean", "default": false, "description": "Raw mode only: bitwise-invert (XOR 0xFF) the recovered bytes, to undo a common obfuscation. Ignored in auto mode." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn hex_preview_formats() {
        assert_eq!(hex_preview(&[0x00, 0xFF, 0x10]), "00ff10");
        assert_eq!(hex_preview(&[]), "");
    }
}
