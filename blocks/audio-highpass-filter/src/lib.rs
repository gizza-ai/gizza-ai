//! gizza-ai/audio-highpass-filter — fetch an audio URL or attachment ref and apply
//! a high-pass filter with ffmpeg's `highpass` biquad: frequencies above the
//! cutoff pass through, low-frequency rumble/hum/handling noise below it is
//! attenuated. Part of the audio-input family (`Input::Audio`). The audio is
//! re-encoded to the chosen format (filtering rewrites samples, so a lossless
//! copy is impossible).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Cutoff/rolloff/format
//! validation and the pure argv builder live in `core`, shared with the page.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_highpass_filter_core::{parse_format, plan, DEFAULT_CUTOFF};
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
    cutoff: Option<f64>,
    #[serde(default)]
    rolloff: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("cutoff")
                .default(80.0)
                .min(10.0)
                .max(2000.0)
                .describe("Cutoff frequency in Hz: everything below this is attenuated, everything above passes. 80 is the standard rumble cut for voice; raise to 100–120 for stubborn hum. Range 10–2000, default 80."),
        )
        .param(
            Param::enumv("rolloff", ["6", "12", "24", "48"])
                .default("12")
                .describe("Filter steepness below the cutoff, in dB/octave: 6 is gentle/transparent, 12 is the natural default for voice, 24 tightens the low end, 48 is a steep brick-wall cut (can thin the sound). Default 12."),
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
struct AudioHighpassFilter;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-highpass-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "High-pass filter an audio file to cut low-frequency rumble",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a high-pass (low-cut) filter to an audio file with ffmpeg's highpass biquad: frequencies above the cutoff pass through, low-frequency rumble/hum/HVAC drone/handling noise below it is attenuated. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). cutoff is the corner frequency in Hz (10–2000, default 80). rolloff is the slope in dB/octave: 6, 12 (default), 24 or 48. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl AudioHighpassFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; cutoff/rolloff/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-highpass-filter")?;
    let cutoff = args.cutoff.unwrap_or(DEFAULT_CUTOFF);
    let rolloff = args.rolloff.as_deref().unwrap_or("12");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates cutoff/rolloff/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, cutoff, rolloff, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-highpass", fmt.ext());
    let for_llm = format!(
        "high-pass filtered {in_filename} (cutoff {cutoff} Hz, {rolloff} dB/oct) → {output_size} bytes {}",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. Note the number
    /// param's default serializes as `80.0` (float), per the documented
    /// drift-guard gotcha.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "cutoff":  { "type": "number", "minimum": 10, "maximum": 2000, "default": 80.0, "description": "Cutoff frequency in Hz: everything below this is attenuated, everything above passes. 80 is the standard rumble cut for voice; raise to 100–120 for stubborn hum. Range 10–2000, default 80." },
                    "rolloff": { "type": "string", "enum": ["6", "12", "24", "48"], "default": "12", "description": "Filter steepness below the cutoff, in dB/octave: 6 is gentle/transparent, 12 is the natural default for voice, 24 tightens the low end, 48 is a steep brick-wall cut (can thin the sound). Default 12." },
                    "format":  { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_highpass_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("interview.wav", "-highpass", "mp3"),
            "interview-highpass.mp3"
        );
    }
}
