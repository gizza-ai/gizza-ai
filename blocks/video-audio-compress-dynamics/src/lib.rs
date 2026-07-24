//! gizza-ai/video-audio-compress-dynamics — fetch a video URL or attachment
//! ref, apply dynamic-range compression to its audio via ffmpeg's
//! `acompressor` filter so quiet and loud passages sit closer together, and
//! return an envelope. The picture is stream-copied (lossless); only the audio
//! is re-encoded (the compressor rewrites samples). The chat schema is derived
//! from `descriptor()` (single source — shared across chat + CLI + page);
//! source-resolution, ffmpeg dispatch, and envelope-building are delegated to
//! `block_utils`. Preset parsing and the pure argv builder live in `core`.
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
use gizza_ai_video_audio_compress_dynamics_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    makeup: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("preset", ["light", "medium", "heavy"])
                .default("medium")
                .describe("How hard to even out the audio: light keeps most of the natural dynamics, medium is a balanced broadcast-style levelling (default), heavy pulls quiet and loud parts close together."),
        )
        .param(
            Param::boolean("makeup")
                .default(true)
                .describe("Apply make-up gain to restore the overall loudness the compressor pulled down. Default on; turn off to tame peaks without raising the level."),
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
struct VideoAudioCompressDynamics;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-compress-dynamics",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Even out a video's loud and quiet audio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply dynamic-range compression to a video's audio so quiet and loud passages sit closer together (evens out the sound; this is NOT file-size compression). The picture is copied losslessly; only the audio is re-encoded. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). preset picks how hard to level: light, medium (default) or heavy. makeup (on by default) restores the overall loudness the compressor pulled down; turn it off to tame peaks without raising the level. The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioCompressDynamics {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; preset validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-compress-dynamics")?;
    let preset = args.preset.as_deref().unwrap_or("medium");
    let makeup = args.makeup.unwrap_or(true);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates the preset).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, preset, makeup).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-dynamics", out_ext);
    let for_llm = format!(
        "compressed audio dynamics of {in_filename} ({preset} preset, makeup {}) ({output_size} bytes {out_mime})",
        if makeup { "on" } else { "off" }
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + preset/makeup), so any future change
    /// to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "preset": { "type": "string", "enum": ["light", "medium", "heavy"], "default": "medium", "description": "How hard to even out the audio: light keeps most of the natural dynamics, medium is a balanced broadcast-style levelling (default), heavy pulls quiet and loud parts close together." },
                    "makeup": { "type": "boolean", "default": true, "description": "Apply make-up gain to restore the overall loudness the compressor pulled down. Default on; turn off to tame peaks without raising the level." }
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
    fn output_filename_uses_dynamics_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-dynamics", "mp4"),
            "clip-dynamics.mp4"
        );
    }
}
