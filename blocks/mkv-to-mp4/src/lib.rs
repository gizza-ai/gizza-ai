//! gizza-ai/mkv-to-mp4 — fetch a Matroska (MKV) URL or attachment ref, convert it
//! into an MP4 container via ffmpeg, and return the media envelope.
//!
//! Default is a lossless `-c copy` stream-copy remux (fast, no quality change —
//! works when the MKV holds H.264/HEVC video + AAC audio); `mode = "transcode"`
//! re-encodes to H.264/AAC as an explicit fallback for MKVs carrying codecs MP4
//! can't hold (VP8/VP9/AV1 video, FLAC/Vorbis/Opus audio). Both modes select only
//! the video + audio streams and drop MKV subtitle/attachment tracks MP4 can't
//! legally carry. Unlike video-transcode / video-compress, which ALWAYS re-encode,
//! the headline here is the stream-copy remux.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page); the handler delegates source-resolution, ffmpeg dispatch,
//! and envelope-building to `block_utils`. The pure `core` argv builder is shared
//! with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, validate_quality_1_100, AssetKind, Input,
    Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_mkv_to_mp4_core::{build_argv, parse_mode, quality_to_crf, DEFAULT_QUALITY};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("mode", ["copy", "transcode"])
                .default("copy")
                .describe(
                    "How to produce the MP4. copy (default) = lossless container remux with \
                     -c copy: near-instant, no quality change, works when the MKV already holds \
                     H.264/HEVC video + AAC audio. transcode = re-encode to H.264/AAC — use for \
                     MKVs whose codecs MP4 can't hold (VP8/VP9/AV1 video, FLAC/Vorbis/Opus audio). \
                     Both modes keep video + audio and drop MKV subtitle/attachment tracks.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(75)
                .describe(
                    "Transcode quality 1-100 (default 75; higher = better quality, larger file). \
                     Maps to ffmpeg's libx264 CRF. Only used when mode=transcode; ignored for \
                     copy (a remux never re-encodes).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MkvToMp4;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mkv-to-mp4",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Matroska (.mkv) video into an MP4 container.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert a Matroska MKV video to MP4. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Default mode 'copy' stream-copies the streams into an MP4 container (lossless, instant) — works when the MKV holds H.264/HEVC + AAC. Use mode 'transcode' to re-encode to H.264/AAC for MKVs whose codecs MP4 can't hold (VP8/VP9/AV1 video, FLAC/Vorbis/Opus audio); quality 1-100 (default 75) maps to CRF and applies only to transcode. Both modes keep video + audio and drop MKV subtitle/attachment tracks MP4 can't carry.",
        parameters = schema_json()
    ),
)]
impl MkvToMp4 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (mode enum + quality 1-100).
    let args: Args = serde_json::from_slice(&body).invalid_args("mkv-to-mp4")?;
    let mode_str = args.mode.as_deref().unwrap_or("copy");
    let mode = parse_mode(mode_str)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid mkv-to-mp4 args: {e}")))?;
    validate_quality_1_100(args.quality, "mkv-to-mp4")?;
    let crf = quality_to_crf(args.quality.unwrap_or(DEFAULT_QUALITY));
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mp4.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mkv");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, mode, crf);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "converted {in_filename} ({in_mime}) to {out_mime} via {mode_str} ({output_size} bytes)"
    );
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
    /// authored schema. `to_schema_json` centralizes the `url`/`ref` wording and
    /// emits `additionalProperties: false`; neither param is required (both have
    /// defaults), so there is no `required` key — only the media `oneOf`.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":    { "type": "string", "enum": ["copy", "transcode"], "default": "copy", "description": "How to produce the MP4. copy (default) = lossless container remux with -c copy: near-instant, no quality change, works when the MKV already holds H.264/HEVC video + AAC audio. transcode = re-encode to H.264/AAC — use for MKVs whose codecs MP4 can't hold (VP8/VP9/AV1 video, FLAC/Vorbis/Opus audio). Both modes keep video + audio and drop MKV subtitle/attachment tracks." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "default": 75, "description": "Transcode quality 1-100 (default 75; higher = better quality, larger file). Maps to ffmpeg's libx264 CRF. Only used when mode=transcode; ignored for copy (a remux never re-encodes)." }
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
    fn output_filename_swaps_extension_to_mp4() {
        assert_eq!(replace_extension("clip.mkv", "mp4"), "clip.mp4");
        assert_eq!(replace_extension("MyMovie.MKV", "mp4"), "MyMovie.mp4");
    }
}
