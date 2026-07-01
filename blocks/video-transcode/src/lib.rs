//! gizza-ai/video-transcode — fetch a video URL or attachment ref, transcode to
//! a different container/codec target via ffmpeg, return envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (format enum, quality 1-100) and the pure `core` argv builder stay shared with
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
    build_media_envelope, mime_to_ext, replace_extension, validate_quality_1_100, AssetKind, Input,
    Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_video_transcode_core::{build_argv, quality_to_crf, DEFAULT_QUALITY};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    format: String,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("format", ["mp4", "webm"])
                .required()
                .describe("Output container format."),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .describe("Quality 1-100 (default 75). Lower = smaller file, lower quality."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoTranscode;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-transcode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transcode a video to mp4 or webm.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Transcode a video to a different format (mp4 or webm). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Quality 1-100 maps to ffmpeg CRF (default 75).",
        parameters = schema_json()
    ),
)]
impl VideoTranscode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — format enum + quality 1-100).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-transcode")?;
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, &args.format).ok_or_else(|| {
            SkillError::InvalidArgs(format!(
                "invalid video-transcode args: format {:?} not supported (mp4|webm)",
                args.format
            ))
        })?;
    validate_quality_1_100(args.quality, "video-transcode")?;
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);
    let crf = quality_to_crf(quality);
    let fmt = gizza_ai_video_transcode_core::parse_format(&args.format)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-transcode args: {e}")))?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output uses the TARGET format ext.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, fmt, crf);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!("transcoded {in_filename} from {in_mime} to {out_mime} ({output_size})");
    build_media_envelope(
        output.as_slice(),
        out_mime,
        filename,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. `to_schema_json`
    /// emits `additionalProperties: false` uniformly (video-transcode's authored
    /// schema lacked it — added below as intentional uniform hardening). The
    /// `url`/`ref` property descriptions are centralized in `to_schema_json`, so
    /// the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format":  { "type": "string", "enum": ["mp4", "webm"], "description": "Output container format." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Quality 1-100 (default 75). Lower = smaller file, lower quality." }
                },
                "additionalProperties": false,
                "required": ["format"],
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
    fn output_filename_swaps_extension() {
        assert_eq!(replace_extension("clip.mov", "mp4"), "clip.mp4");
        assert_eq!(replace_extension("clip.mp4", "webm"), "clip.webm");
    }
}
