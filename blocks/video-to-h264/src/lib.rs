//! gizza-ai/video-to-h264 — fetch any video URL or attachment ref, force-transcode
//! it to the most universally playable form (H.264 High-profile MP4, yuv420p,
//! +faststart, AAC audio), and return the media envelope.
//!
//! Unlike mov-to-mp4 (which REMUXES when it can and keeps the source
//! codec/pixel-format), video-transcode (container/codec switch), and
//! video-compress (a size/quality knob), this tool ALWAYS re-encodes and ALWAYS
//! pins `-profile:v` + `-pix_fmt yuv420p` + `-movflags +faststart`. That forced
//! normalize is the value: it makes "won't-play" 10-bit / 4:2:2 / HEVC / VP9 /
//! AV1 clips decode on essentially every browser, phone, TV and old player.
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
use gizza_ai_video_to_h264_core::{build_argv, parse_profile, quality_to_crf, DEFAULT_QUALITY};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("profile", ["high", "main", "baseline"])
                .default("high")
                .describe(
                    "H.264 profile the output is pinned to. high (default) = best compression, \
                     plays on every browser and modern device (2012+). main = slightly wider \
                     legacy reach, marginally larger. baseline = maximum compatibility with very \
                     old / embedded players (no B-frames or CABAC), largest file — use only when \
                     something genuinely refuses high.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(75)
                .describe(
                    "Encode quality 1-100 (default 75; higher = better quality, larger file). \
                     Maps to ffmpeg's libx264 CRF (100 = visually lossless CRF 18, 75 ≈ CRF 24, \
                     1 = small CRF 40). Output is always re-encoded — there is no lossless copy.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToH264;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// the native unit tests can still compile.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-h264",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Force-transcode any video to universally-playable H.264 MP4.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Force-transcode any video to the most universally playable form: H.264 (libx264) in an MP4 container, yuv420p 8-bit 4:2:0 chroma, +faststart, and AAC audio. Use this to make a clip that 'won't play' (10-bit, 4:2:2/4:4:4, HEVC, VP9, AV1, .webm/.mkv/.avi) decode on essentially every browser, phone, TV and old player. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). profile is high|main|baseline (default high; baseline = max compatibility with very old/embedded players). quality 1-100 (default 75) maps to libx264 CRF. Always re-encodes — for a lossless container remux use mov-to-mp4 instead.",
        parameters = schema_json()
    ),
)]
impl VideoToH264 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (profile enum + quality 1-100).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-to-h264")?;
    let profile_str = args.profile.as_deref().unwrap_or("high");
    let profile = parse_profile(profile_str)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-h264 args: {e}")))?;
    validate_quality_1_100(args.quality, "video-to-h264")?;
    let crf = quality_to_crf(args.quality.unwrap_or(DEFAULT_QUALITY));
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output is always out.mp4.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, profile, crf);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope.
    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "normalized {in_filename} ({in_mime}) to {out_mime} — H.264 {profile_str} profile, \
         yuv420p, faststart ({output_size} bytes)"
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
                    "profile": { "type": "string", "enum": ["high", "main", "baseline"], "default": "high", "description": "H.264 profile the output is pinned to. high (default) = best compression, plays on every browser and modern device (2012+). main = slightly wider legacy reach, marginally larger. baseline = maximum compatibility with very old / embedded players (no B-frames or CABAC), largest file — use only when something genuinely refuses high." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "default": 75, "description": "Encode quality 1-100 (default 75; higher = better quality, larger file). Maps to ffmpeg's libx264 CRF (100 = visually lossless CRF 18, 75 ≈ CRF 24, 1 = small CRF 40). Output is always re-encoded — there is no lossless copy." }
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
        assert_eq!(replace_extension("clip.webm", "mp4"), "clip.mp4");
        assert_eq!(replace_extension("Recording.MKV", "mp4"), "Recording.mp4");
    }
}
