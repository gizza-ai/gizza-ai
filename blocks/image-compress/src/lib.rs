//! gizza-ai/image-compress — fetch an image URL or attachment ref, re-encode it
//! at a chosen quality to shrink its file size, keeping the SAME format.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (quality 1-100) and the pure `core` argv builder (`plan_compress`) stay here.
//! See docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, validate_quality_1_100, AssetKind,
    Input, Param, SkillError, SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_compress_core::plan_compress;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_QUALITY: u8 = 80;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::integer("quality")
            .min(1.0)
            .max(100.0)
            .describe("Output quality 1-100 (default 80). Lower = smaller file."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// One-line summary for the LLM: what was compressed, at what quality, and the
/// byte delta.
fn summary(source: &str, quality: u8, input_size: usize, output_size: usize, mime: &str) -> String {
    format!("compressed {source} at quality {quality}: {input_size} → {output_size} bytes ({mime})")
}

#[cfg(target_arch = "wasm32")]
struct ImageCompress;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-encode an image at a chosen quality to shrink its file size, keeping the same format",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Compress (re-encode) an image at a chosen quality to shrink its file size, keeping the same format (jpg/png/webp). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Lower quality = smaller file. For JPEG/WebP this trades visual fidelity for size; PNG is lossless so quality only tunes compression effort.",
        parameters = schema_json()
    ),
)]
impl ImageCompress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — quality 1-100).
    let args: Args = serde_json::from_slice(&body).invalid_args("image-compress")?;
    validate_quality_1_100(args.quality, "image-compress")?;
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let input_size = input_bytes.len();

    // 3. Build ffmpeg argv via the shared core (keeps the same format, infers
    //    the encoder flag from the input extension).
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let (argv, ffmpeg_out) = plan_compress(quality, &ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope (same mime as the input — format is unchanged).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-compressed", ext);
    let for_llm = summary(&in_filename, quality, input_size, output_size, &mime);
    build_media_envelope(&output, &mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// now emits `additionalProperties: false` uniformly (image-compress's
    /// authored schema lacked it — added below as intentional uniform hardening).
    /// The `url`/`ref` property descriptions are centralized in `to_schema_json`,
    /// so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Output quality 1-100 (default 80). Lower = smaller file." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn summary_reports_byte_delta() {
        let s = summary("cat.jpg", 60, 12000, 7000, "image/jpeg");
        assert!(s.contains("cat.jpg"));
        assert!(s.contains("quality 60"));
        assert!(s.contains("12000"));
        assert!(s.contains("7000"));
    }

    #[test]
    fn compressed_filename_keeps_extension() {
        assert_eq!(
            filename_with_suffix("cat.png", "-compressed", "png"),
            "cat-compressed.png"
        );
        assert_eq!(
            filename_with_suffix("photo.jpg", "-compressed", "jpg"),
            "photo-compressed.jpg"
        );
    }
}
