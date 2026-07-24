//! gizza-ai/audio-fit-to-length — fetch an audio URL or attachment ref and fit it
//! to an exact target duration by padding with silence (when shorter) or trimming
//! (when longer). Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Bounds validation and the
//! pure argv builder live in `core`, shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_fit_to_length_core::{fmt_num, parse_format, plan_fit, DEFAULT_DURATION_S};
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    pad: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("duration")
                .min(0.0)
                .max(3600.0)
                .default(30.0)
                .describe("Exact target length in seconds (default 30, max 3600). If the clip is shorter it is padded with silence; if longer it is trimmed. Example: 60 makes any clip exactly one minute."),
        )
        .param(
            Param::enumv("pad", ["end", "start"])
                .default("end")
                .describe("Where silence is added when the clip is shorter than the target: 'end' (default) appends silence after the clip; 'start' prepends it before the clip. When the clip is longer than the target it is trimmed from the end regardless."),
        )
        .param(
            Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"])
                .default("mp3")
                .describe("Output audio format. Default mp3 (192 kbps)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioFitToLength;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-fit-to-length",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pad with silence or trim an audio file to an exact duration",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Fit an audio clip to an exact total length. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call) plus duration (seconds, default 30, max 3600). If the clip is shorter than duration it is padded with silence to reach exactly that length; if longer it is trimmed to exactly that length. pad = end (default, silence after the clip) or start (silence before the clip); a longer clip is trimmed from the end either way. The audio is re-encoded, so silence joins cleanly (no container-copy clicks). Output format mp3 (192 kbps, default), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioFitToLength {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; bounds validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-fit-to-length")?;
    let duration = args.duration.unwrap_or(DEFAULT_DURATION_S);
    let pad = args.pad.as_deref().unwrap_or("end");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates + picks the filter).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_fit(&ffmpeg_in, duration, pad, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fit", fmt.ext());
    let for_llm = format!(
        "fit {in_filename} to {} s ({output_size} bytes {})",
        fmt_num(duration),
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Audio
    /// wording). The number param's default serializes as a float (`30.0`);
    /// whole-number bounds render as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "duration": { "type": "number", "minimum": 0, "maximum": 3600, "default": 30.0, "description": "Exact target length in seconds (default 30, max 3600). If the clip is shorter it is padded with silence; if longer it is trimmed. Example: 60 makes any clip exactly one minute." },
                    "pad":      { "type": "string", "enum": ["end", "start"], "default": "end", "description": "Where silence is added when the clip is shorter than the target: 'end' (default) appends silence after the clip; 'start' prepends it before the clip. When the clip is longer than the target it is trimmed from the end regardless." },
                    "format":   { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_fit_suffix_and_format_ext() {
        assert_eq!(filename_with_suffix("intro.wav", "-fit", "mp3"), "intro-fit.mp3");
        assert_eq!(
            filename_with_suffix("ad spot.m4a", "-fit", "flac"),
            "ad spot-fit.flac"
        );
    }
}
