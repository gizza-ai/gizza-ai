//! gizza-ai/video-trim — fetch a video URL or attachment ref, trim to
//! `[start, start+duration]` via a stream-copy (no re-encode), return envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (start >= 0, duration > 0) and the pure `core` argv builder stay shared with
//! the page. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_trim_core::{build_argv, OUT_NAME};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    start: f64,
    duration: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("start")
                .required()
                .min(0.0)
                .describe("Start time in seconds."),
        )
        .param(
            Param::number("duration")
                .required()
                .min(0.0)
                .describe("Duration in seconds."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoTrim;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-trim",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim a video to a [start, start+duration] window",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Trim a video to a [start, start+duration] window using stream-copy (no re-encode). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Output is mp4. Stream-copy preserves the source codecs and is fast — but requires the source streams be mp4-compatible (h264/aac); otherwise ffmpeg will fail with a clear error.",
        parameters = schema_json()
    ),
)]
impl VideoTrim {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — start >= 0, duration > 0, both finite).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-trim")?;
    if args.start < 0.0 || !args.start.is_finite() {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-trim args: start must be >= 0 and finite, got {}",
            args.start
        )));
    }
    if args.duration <= 0.0 || !args.duration.is_finite() {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-trim args: duration must be > 0 and finite, got {}",
            args.duration
        )));
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always mp4 (stream-copy).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = OUT_NAME.to_string();
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.start, args.duration);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope. Output is mp4.
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-trimmed", "mp4");
    let for_llm = format!(
        "trimmed {} to [{}s, {}s+{}s] ({} bytes mp4)",
        in_filename, args.start, args.start, args.duration, output_size
    );
    build_media_envelope(&output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. Two intentional,
    /// documented normalizations apply:
    ///   - `to_schema_json` emits `additionalProperties: false` uniformly
    ///     (video-trim's authored schema lacked it — added below as uniform
    ///     hardening).
    ///   - `duration`'s authored `exclusiveMinimum: 0` becomes `minimum: 0`: the
    ///     descriptor expresses inclusive bounds only, and the two differ only at
    ///     exactly `duration == 0`, which `run()` already rejects with a clear
    ///     error — so there is no runtime-behavior change.
    /// The `url`/`ref` property descriptions are centralized in `to_schema_json`,
    /// so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "start":    { "type": "number", "minimum": 0, "description": "Start time in seconds." },
                    "duration": { "type": "number", "minimum": 0, "description": "Duration in seconds." }
                },
                "additionalProperties": false,
                "required": ["start", "duration"],
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
    fn output_filename_uses_trimmed_suffix_and_mp4() {
        assert_eq!(
            filename_with_suffix("clip.webm", "-trimmed", "mp4"),
            "clip-trimmed.mp4"
        );
    }
}
