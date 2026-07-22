//! gizza-ai/png-chunk-stripper — losslessly remove ancillary PNG chunks
//! (metadata, text, EXIF, color hints) by walking the chunk stream at the byte
//! level and copying every kept chunk verbatim. IDAT/pixels are never decoded or
//! re-encoded, so the displayed image is bit-for-bit identical. Returns a PNG.
//!
//! Pipeline: resolve the source PNG (url/ref) → `core::strip` (pure Rust, no
//! image decode) → media envelope with the cleaned PNG bytes.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker (no ffmpeg,
//! no image decode). Surfaces: chat + CLI. No standalone page — image-bytes
//! output has no page render mode (same shape as png-optimizer / strip-exif).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    filename_with_suffix, AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_png_chunk_stripper_core::{strip, Mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    keep: String,
}
fn default_mode() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["all", "metadata", "text"])
                .default("all")
                .describe("Which ancillary chunks to remove: all (default — every ancillary chunk incl. color hints gAMA/cHRM/sRGB/iCCP; smallest file, but on-screen colors may shift), metadata (text tEXt/zTXt/iTXt + EXIF eXIf + timestamp tIME; keeps color-management and physical-size chunks so appearance and DPI are preserved), or text (only the privacy carriers — text chunks + EXIF). Critical chunks (IHDR/PLTE/IDAT/IEND) and transparency (tRNS) are always kept. Default \"all\"."),
        )
        .param(
            Param::string("keep")
                .describe("Comma-separated list of 4-character PNG chunk types to always preserve, overriding mode (e.g. \"iCCP,pHYs\"). Case-sensitive. Leave blank to keep nothing extra."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Parse the comma-separated `keep` field into a list of chunk-type strings.
fn parse_keep(keep: &str) -> Vec<String> {
    keep.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(target_arch = "wasm32")]
struct PngChunkStripper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/png-chunk-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Losslessly strip ancillary PNG chunks (metadata/text/color hints), pixels unchanged",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Losslessly remove ancillary chunks from a PNG by walking the chunk stream at the byte level — the pixel data (IDAT) is never decoded or re-encoded, so the displayed image is bit-for-bit identical. mode is all|metadata|text (default all): all strips every ancillary chunk including color hints (gAMA/cHRM/sRGB/iCCP) for the smallest file; metadata strips text/EXIF/timestamps but keeps color-management + physical-size chunks so appearance and DPI are preserved; text strips only the text and EXIF privacy carriers. keep is a comma-separated list of 4-character chunk types to always preserve (e.g. iCCP,pHYs), overriding mode. Critical chunks (IHDR/PLTE/IDAT/IEND) and transparency (tRNS) are always kept. Non-PNG input is rejected. Provide the PNG as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl PngChunkStripper {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("png-chunk-stripper")?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let keep = parse_keep(&args.keep);

    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let res = strip(&input_bytes, mode, &keep).map_err(SkillError::InvalidArgs)?;

    let filename = filename_with_suffix(&in_filename, "-stripped", "png");
    let for_llm = res.summary();
    build_media_envelope(&res.bytes, "image/png", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["all", "metadata", "text"], "default": "all", "description": "Which ancillary chunks to remove: all (default — every ancillary chunk incl. color hints gAMA/cHRM/sRGB/iCCP; smallest file, but on-screen colors may shift), metadata (text tEXt/zTXt/iTXt + EXIF eXIf + timestamp tIME; keeps color-management and physical-size chunks so appearance and DPI are preserved), or text (only the privacy carriers — text chunks + EXIF). Critical chunks (IHDR/PLTE/IDAT/IEND) and transparency (tRNS) are always kept. Default \"all\"." },
                    "keep": { "type": "string", "description": "Comma-separated list of 4-character PNG chunk types to always preserve, overriding mode (e.g. \"iCCP,pHYs\"). Case-sensitive. Leave blank to keep nothing extra." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn parse_keep_splits_and_trims() {
        assert_eq!(parse_keep(" iCCP , pHYs "), vec!["iCCP", "pHYs"]);
        assert!(parse_keep("").is_empty());
        assert!(parse_keep("  ,  ").is_empty());
    }

    #[test]
    fn clean_filename_uses_png_extension() {
        assert_eq!(
            filename_with_suffix("photo.png", "-stripped", "png"),
            "photo-stripped.png"
        );
    }
}
