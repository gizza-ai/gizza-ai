//! gizza-ai/video-audio-fade — fetch a video URL or attachment ref, add an
//! audio-only fade-in at the start and/or fade-out at the end, and return an
//! envelope. The picture is stream-copied (lossless, untouched); only the audio
//! is re-encoded (its samples are being ramped). The chat schema is derived
//! from `descriptor()` (single source — shared across chat + CLI + page);
//! source-resolution, ffmpeg dispatch, and envelope-building are delegated to
//! `block_utils`. Fade validation and the pure argv builder live in `core`.
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
use gizza_ai_video_audio_fade_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    fade_in: f64,
    #[serde(default)]
    fade_out: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("fade_in")
                .min(0.0)
                .max(30.0)
                .default(0.0)
                .describe("Length in seconds of the fade-in ramped up from silence at the START of the audio. 0 skips the fade-in. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0."),
        )
        .param(
            Param::number("fade_out")
                .min(0.0)
                .max(30.0)
                .default(0.0)
                .describe("Length in seconds of the fade-out ramped down to silence at the END of the audio. 0 skips the fade-out. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0."),
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
struct VideoAudioFade;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-fade",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fade a video's audio in at the start and/or out at the end",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add an audio-only fade-in at the start and/or fade-out at the end of a video, without touching the picture (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). fade_in and fade_out are lengths in seconds (0–30); 0 skips that side, and at least one must be greater than 0. The output keeps the input container (mp4→mp4, webm→webm; webm audio becomes Opus, otherwise AAC). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioFade {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; fade validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-fade")?;

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates the fade lengths).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.fade_in, args.fade_out).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-faded", out_ext);
    let for_llm = format!(
        "faded the audio of {in_filename} (fade-in {} s, fade-out {} s) with the picture untouched ({output_size} bytes {out_mime})",
        args.fade_in, args.fade_out
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + fade_in/fade_out), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fade_in":  { "type": "number", "minimum": 0, "maximum": 30, "default": 0.0, "description": "Length in seconds of the fade-in ramped up from silence at the START of the audio. 0 skips the fade-in. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0." },
                    "fade_out": { "type": "number", "minimum": 0, "maximum": 30, "default": 0.0, "description": "Length in seconds of the fade-out ramped down to silence at the END of the audio. 0 skips the fade-out. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0." }
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
    fn output_filename_uses_faded_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-faded", "mp4"),
            "clip-faded.mp4"
        );
    }
}
