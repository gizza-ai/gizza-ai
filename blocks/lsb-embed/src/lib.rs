//! gizza-ai/lsb-embed — hide a secret text message inside an image's pixels
//! using LSB (least-significant-bit) steganography, returning a stego PNG.
//!
//! Pure-Rust (the `image` crate + a bit-packer in core), so unlike the ffmpeg
//! tools it runs on ALL backends including the chat Service Worker. Pipeline:
//! resolve the carrier image (URL fetch or attachment ref) → `core::embed`
//! writes the message bits into the low bits of R/G/B channels → base64 PNG
//! envelope. The output MUST be PNG (lossless) so the hidden bits survive.
//!
//! Surfaces: chat + CLI. No standalone page — a pure-Rust image-bytes output has
//! no page render mode (same as add-text-to-image / code-screenshot).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{build_media_envelope, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    message: String,
    #[serde(default = "default_bits")]
    bits_per_channel: i64,
}
fn default_bits() -> i64 {
    1
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("message")
                .required()
                .describe("The secret text to hide inside the image."),
        )
        .param(
            Param::integer("bits_per_channel")
                .default(1)
                .min(1.0)
                .max(4.0)
                .describe("Low bits to use per colour channel, 1-4 (default 1). Higher hides more text but is more visible."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LsbEmbed;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lsb-embed",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hide a secret message inside an image (LSB steganography)",
    requires = ["wafer-run/network"],
    skill(
        description = "Hide a secret text message inside an image using LSB (least-significant-bit) steganography and return a stego PNG that looks identical to the original. Set bits_per_channel 1-4 (default 1; higher hides more text but is slightly more visible). The output is always a lossless PNG so the hidden data survives. Provide the carrier image as either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl LsbEmbed {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("lsb-embed: {e}")))?;
    if args.message.is_empty() {
        return Err(SkillError::InvalidArgs("message is required".into()));
    }
    if !(1..=4).contains(&args.bits_per_channel) {
        return Err(SkillError::InvalidArgs(
            "bits_per_channel must be between 1 and 4".into(),
        ));
    }

    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let png = gizza_ai_lsb_embed_core::embed(
        &bytes,
        args.message.as_bytes(),
        args.bits_per_channel as u8,
    )
    .map_err(SkillError::InvalidArgs)?;

    let stem = in_filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&in_filename);
    let filename = format!("{stem}-stego.png");
    let for_llm = format!(
        "hid a {}-byte message inside {in_filename} → {}-byte stego PNG ({filename})",
        args.message.len(),
        png.len()
    );
    build_media_envelope(&png, "image/png", filename, for_llm, MAX_BYTES)
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
                    "message":          { "type": "string", "description": "The secret text to hide inside the image." },
                    "bits_per_channel": { "type": "integer", "minimum": 1, "maximum": 4, "default": 1, "description": "Low bits to use per colour channel, 1-4 (default 1). Higher hides more text but is more visible." }
                },
                "required": ["message"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
