//! gizza-ai/video-audio-gain — fetch a video URL or attachment ref, raise or
//! lower its audio volume via ffmpeg, and return an envelope. The picture is
//! stream-copied (lossless); only the audio is re-encoded (the `volume` filter
//! rewrites samples). The chat schema is derived from `descriptor()` (single
//! source — shared across chat + CLI + page); source-resolution, ffmpeg
//! dispatch, and envelope-building are delegated to `block_utils`. Amount/unit
//! validation and the pure argv builder live in `core`.
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
use gizza_ai_video_audio_gain_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    amount: f64,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    limiter: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("amount")
                .required()
                .min(-60.0)
                .max(60.0)
                .describe("How much to change the audio volume: with unit=db, decibels (6 boosts, -6 cuts, 0 not allowed); with unit=factor, a multiplier in (0, 16] (2 doubles, 0.5 halves)."),
        )
        .param(
            Param::enumv("unit", ["db", "factor"])
                .default("db")
                .describe("How amount is interpreted: decibels (default) or a linear factor."),
        )
        .param(
            Param::boolean("limiter")
                .default(true)
                .describe("Cap peaks at 0 dBFS (alimiter) so boosts don't clip. Default on; disable for exact linear gain."),
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
struct VideoAudioGain;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-gain",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Raise or lower a video's audio volume",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Raise or lower the audio volume of a video, keeping the picture untouched (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). amount is in decibels by default (6 boosts, -6 cuts; range ±60) or a linear factor with unit=factor (2 doubles, 0.5 halves; range (0,16]). A peak limiter (on by default) caps output at 0 dBFS so boosts don't clip. The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioGain {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; amount/unit validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-gain")?;
    let unit = args.unit.as_deref().unwrap_or("db");
    let limiter = args.limiter.unwrap_or(true);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates amount for the unit).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.amount, unit, limiter).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-gain", out_ext);
    let change = if unit == "factor" {
        format!("x{}", args.amount)
    } else {
        format!("{:+} dB", args.amount)
    };
    let for_llm =
        format!("adjusted audio volume of {in_filename} by {change} ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + amount/unit/limiter), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "amount":  { "type": "number", "minimum": -60, "maximum": 60, "description": "How much to change the audio volume: with unit=db, decibels (6 boosts, -6 cuts, 0 not allowed); with unit=factor, a multiplier in (0, 16] (2 doubles, 0.5 halves)." },
                    "unit":    { "type": "string", "enum": ["db", "factor"], "default": "db", "description": "How amount is interpreted: decibels (default) or a linear factor." },
                    "limiter": { "type": "boolean", "default": true, "description": "Cap peaks at 0 dBFS (alimiter) so boosts don't clip. Default on; disable for exact linear gain." }
                },
                "required": ["amount"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_gain_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-gain", "mp4"), "clip-gain.mp4");
    }
}
