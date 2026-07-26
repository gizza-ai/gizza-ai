//! gizza-ai/video-audio-bitrate-set — fetch a video URL or attachment ref,
//! re-encode ONLY its audio track at a chosen constant bitrate via ffmpeg, and
//! return an envelope. The picture is stream-copied (`-c:v copy`, lossless — the
//! video is never touched or degraded); only the audio is re-encoded, so an
//! oversized soundtrack shrinks while the video stays byte-identical. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page); source-resolution, ffmpeg dispatch, and envelope-building are
//! delegated to `block_utils`. Bitrate parsing and the pure argv builder live in
//! `core`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_audio_bitrate_set_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    bitrate: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::enumv("bitrate", ["64", "96", "128", "160", "192", "256", "320"])
            .default("128")
            .describe("Target audio bitrate in kbps (constant/CBR). Lower = smaller file, lower quality; higher = larger, better. Default 128 (good for stereo music/speech); use 64–96 for voice/podcast, 192–320 to keep music quality. Only the audio is re-encoded; the video is copied untouched."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoAudioBitrateSet;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-bitrate-set",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-encode only a video's audio at a chosen bitrate to shrink an oversized soundtrack",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Re-encode ONLY a video's audio track at a chosen constant bitrate to shrink an oversized soundtrack, keeping the picture untouched (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). bitrate is in kbps (64/96/128/160/192/256/320; default 128) — use 64–96 for speech/podcasts, 192–320 to preserve music. The audio codec matches the container (AAC for mp4/mov/mkv, Opus for webm). The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioBitrateSet {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; bitrate parsing/validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-bitrate-set")?;
    let bitrate = args.bitrate.as_deref().unwrap_or("128");

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates the bitrate).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, bitrate).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-audio", out_ext);
    let for_llm = format!(
        "re-encoded audio of {in_filename} to {bitrate} kbps ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the bitrate enum), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "bitrate": { "type": "string", "enum": ["64", "96", "128", "160", "192", "256", "320"], "default": "128", "description": "Target audio bitrate in kbps (constant/CBR). Lower = smaller file, lower quality; higher = larger, better. Default 128 (good for stereo music/speech); use 64–96 for voice/podcast, 192–320 to keep music quality. Only the audio is re-encoded; the video is copied untouched." }
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
    fn output_filename_uses_audio_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-audio", "mp4"),
            "clip-audio.mp4"
        );
    }
}
