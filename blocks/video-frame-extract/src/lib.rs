//! gizza-ai/video-frame-extract — fetch a video URL or attachment ref, extract a
//! single frame at a given timestamp, return it as a PNG envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure `core` argv
//! builder is shared with the standalone web page. The input is a video but the
//! output is always a PNG image, so the page is `format="image"`. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard below can exercise
// them. See image-resize for the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_frame_extract_core::{build_argv, validate_timestamp, OUTPUT_NAME};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// The output is always a PNG image, regardless of the input video container.
const OUTPUT_MIME: &str = "image/png";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    timestamp: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::number("timestamp")
            .required()
            .min(0.0)
            .describe("Timestamp in seconds."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoFrameExtract;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-frame-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a single frame from a video at a given timestamp",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Extract a single frame from a video at the given timestamp (seconds), output as PNG. The PNG is naturally chainable into image-resize, image-crop, or image-convert via ref. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl VideoFrameExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — timestamp must be >= 0 and finite).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-frame-extract")?;
    validate_timestamp(args.timestamp)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-frame-extract args: {e}")))?;

    // 2. Resolve source — URL fetch or attachment lookup (input is a video).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always PNG.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = OUTPUT_NAME.to_string();
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.timestamp);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the extracted frame is an image/png.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, &format!("-frame-{}", args.timestamp), "png");
    let for_llm = format!(
        "extracted frame at {}s from {} (PNG, {} bytes)",
        args.timestamp, in_filename, output_size
    );
    build_media_envelope(&output, OUTPUT_MIME, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// emits `additionalProperties: false` uniformly (the authored schema lacked
    /// it — added below as intentional uniform hardening) and centralizes the
    /// `url`/`ref` property descriptions (the authored schema left them blank —
    /// the expected JSON uses the shared `Input::Video` wording).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "timestamp": { "type": "number", "minimum": 0, "description": "Timestamp in seconds." }
                },
                "additionalProperties": false,
                "required": ["timestamp"],
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
    fn output_filename_uses_frame_timestamp_suffix_and_png() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-frame-5", "png"),
            "clip-frame-5.png"
        );
    }
}
