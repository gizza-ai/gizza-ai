//! gizza-ai/video-audio-to-mono — fetch a video URL or attachment ref, downmix
//! its audio to a single mono channel via ffmpeg, and return an envelope. The
//! picture is stream-copied (lossless); only the audio is re-encoded. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page); source-resolution, ffmpeg dispatch, and envelope-building are
//! delegated to `block_utils`. Channel/bitrate/sample-rate validation and the
//! pure argv builder live in `core`.
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
use gizza_ai_video_audio_to_mono_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

const DEFAULT_BITRATE_KBPS: f64 = 128.0;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    bitrate: Option<f64>,
    #[serde(default)]
    sample_rate: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("channel", ["mix", "left", "right", "difference"])
                .default("mix")
                .describe("Which source audio ends up in the mono track: mix downmixes every channel (default, correct for stereo and 5.1); left/right keep just that side, the fix for a recording where only one channel has usable sound; difference is L-R, which cancels centred content."),
        )
        .param(
            Param::integer("bitrate")
                .default(128)
                .min(16.0)
                .max(320.0)
                .describe("Audio bitrate of the mono track in kbps (16-320, default 128). Mono needs about half of what the same stereo track did, so lowering this is the main way to shrink the file: 64 is fine for speech, 32 for voice memos."),
        )
        .param(
            Param::enumv(
                "sample_rate",
                ["keep", "48000", "44100", "32000", "22050", "16000"],
            )
            .default("keep")
            .describe("Audio sample rate in Hz, or keep (default) to leave the source rate alone. Lowering it shrinks the file further; 16000 is speech-grade. WebM output snaps to the nearest rate libopus accepts (8000/12000/16000/24000/48000)."),
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
struct VideoAudioToMono;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-to-mono",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Downmix a video's audio to a single mono channel",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Downmix a video's audio to one mono channel, keeping the picture untouched (the video stream is copied losslessly; only the audio is re-encoded). Fixes a clip where sound only came out of one side, and shrinks the file because mono needs about half the bits of stereo. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). channel picks what feeds the mono track: mix (default), left, right, or difference (L-R). bitrate sets the mono track's kbps (16-320, default 128) and sample_rate optionally resamples (keep|48000|44100|32000|22050|16000). The output keeps the input container (mp4/mov/mkv stay AAC, webm stays Opus). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioToMono {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; channel/bitrate/sample-rate validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-to-mono")?;
    let channel = args.channel.as_deref().unwrap_or("mix");
    let bitrate = args.bitrate.unwrap_or(DEFAULT_BITRATE_KBPS);
    let sample_rate = args.sample_rate.as_deref().unwrap_or("keep");

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates every param).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, channel, bitrate, sample_rate).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-mono", out_ext);
    let rate = if sample_rate == "keep" {
        String::new()
    } else {
        format!(", {sample_rate} Hz")
    };
    let for_llm = format!(
        "downmixed the audio of {in_filename} to mono (channel {channel}, {bitrate} kbps{rate}) \
         ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + channel/bitrate/sample_rate), so any
    /// future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "channel":     { "type": "string", "enum": ["mix", "left", "right", "difference"], "default": "mix", "description": "Which source audio ends up in the mono track: mix downmixes every channel (default, correct for stereo and 5.1); left/right keep just that side, the fix for a recording where only one channel has usable sound; difference is L-R, which cancels centred content." },
                    "bitrate":     { "type": "integer", "minimum": 16, "maximum": 320, "default": 128, "description": "Audio bitrate of the mono track in kbps (16-320, default 128). Mono needs about half of what the same stereo track did, so lowering this is the main way to shrink the file: 64 is fine for speech, 32 for voice memos." },
                    "sample_rate": { "type": "string", "enum": ["keep", "48000", "44100", "32000", "22050", "16000"], "default": "keep", "description": "Audio sample rate in Hz, or keep (default) to leave the source rate alone. Lowering it shrinks the file further; 16000 is speech-grade. WebM output snaps to the nearest rate libopus accepts (8000/12000/16000/24000/48000)." }
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
    fn output_filename_uses_mono_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-mono", "mp4"),
            "clip-mono.mp4"
        );
        assert_eq!(
            filename_with_suffix("interview.webm", "-mono", "webm"),
            "interview-mono.webm"
        );
    }
}
