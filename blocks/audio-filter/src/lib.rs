//! gizza-ai/audio-filter — fetch an audio URL or attachment ref and apply one of
//! four classic filters with ffmpeg: **low-pass** (`lowpass`), **high-pass**
//! (`highpass`), **band-pass** (`bandpass`), or **notch** / band-reject
//! (`bandreject`). Part of the audio-input family (`Input::Audio`). The audio is
//! re-encoded to the chosen format (filtering rewrites samples, so a lossless
//! copy is impossible).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Filter-type/frequency/width/
//! format validation and the pure argv builder live in `core`, shared with the
//! page.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_filter_core::{parse_format, plan, DEFAULT_FREQ, DEFAULT_WIDTH};
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
    #[serde(default, rename = "type")]
    filter_type: Option<String>,
    #[serde(default)]
    frequency: Option<f64>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("type", ["lowpass", "highpass", "bandpass", "notch"])
                .default("lowpass")
                .describe("Filter shape. lowpass keeps lows and cuts highs above the frequency; highpass keeps highs and cuts lows below it; bandpass keeps only a band centred on the frequency (width wide); notch removes a band centred on the frequency (width wide). Default lowpass."),
        )
        .param(
            Param::number("frequency")
                .default(1000.0)
                .min(20.0)
                .max(20000.0)
                .describe("Corner frequency for low-/high-pass, or the band centre for band-pass/notch, in Hz. E.g. 3000 to tame highs (lowpass), 80 to cut rumble (highpass), 60 to kill mains hum (notch). Range 20–20000, default 1000."),
        )
        .param(
            Param::number("width")
                .default(200.0)
                .min(1.0)
                .max(10000.0)
                .describe("Band width in Hz for band-pass/notch only (ignored by low-/high-pass). A narrow width (e.g. 20) makes a surgical notch; a wide one (e.g. 3000) passes a broad band. Range 1–10000, default 200."),
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
struct AudioFilter;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a low-pass, high-pass, band-pass, or notch filter to audio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a classic audio filter with ffmpeg. type selects the shape: lowpass (cut highs above the frequency), highpass (cut lows below it), bandpass (keep only a band centred on frequency, width Hz wide), or notch/band-reject (remove that band). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). frequency is the corner/centre in Hz (20–20000, default 1000). width is the band width in Hz for bandpass/notch only (1–10000, default 200; ignored by low-/high-pass). Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl AudioFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; filter-type/frequency/width/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-filter")?;
    let filter_type = args.filter_type.as_deref().unwrap_or("lowpass");
    let frequency = args.frequency.unwrap_or(DEFAULT_FREQ);
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates filter/frequency/width/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, filter_type, frequency, width, format)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-filtered", fmt.ext());
    let for_llm = format!(
        "{filter_type} filtered {in_filename} ({frequency} Hz) → {output_size} bytes {}",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. Note the number
    /// params' defaults serialize as floats (`1000.0`, `200.0`), per the
    /// documented drift-guard gotcha.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "type":      { "type": "string", "enum": ["lowpass", "highpass", "bandpass", "notch"], "default": "lowpass", "description": "Filter shape. lowpass keeps lows and cuts highs above the frequency; highpass keeps highs and cuts lows below it; bandpass keeps only a band centred on the frequency (width wide); notch removes a band centred on the frequency (width wide). Default lowpass." },
                    "frequency": { "type": "number", "minimum": 20, "maximum": 20000, "default": 1000.0, "description": "Corner frequency for low-/high-pass, or the band centre for band-pass/notch, in Hz. E.g. 3000 to tame highs (lowpass), 80 to cut rumble (highpass), 60 to kill mains hum (notch). Range 20–20000, default 1000." },
                    "width":     { "type": "number", "minimum": 1, "maximum": 10000, "default": 200.0, "description": "Band width in Hz for band-pass/notch only (ignored by low-/high-pass). A narrow width (e.g. 20) makes a surgical notch; a wide one (e.g. 3000) passes a broad band. Range 1–10000, default 200." },
                    "format":    { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_filtered_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("interview.wav", "-filtered", "mp3"),
            "interview-filtered.mp3"
        );
    }
}
