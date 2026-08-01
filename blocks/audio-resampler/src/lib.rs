//! gizza-ai/audio-resampler — fetch an audio URL or attachment ref and change
//! its SAMPLE RATE (Hz) with ffmpeg's high-quality swresample resampler, writing
//! the result to wav/flac/mp3/ogg/m4a. Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Rate validation, format
//! parsing, and the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call requiring ::new()). The supporting imports, constants, and
// Args type are only used inside that wasm32-gated impl, so they look "unused"
// during native unit tests. `descriptor()`/`schema_json()` stay native-
// compilable so the drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_audio_resampler_core::{parse_format, plan_resample, DEFAULT_FORMAT};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    rate: u32,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::integer("rate")
                .required()
                .min(3000.0)
                .max(384000.0)
                .describe("Target sample rate in Hz. Common values: 8000/16000 (speech), 22050, 32000, 44100 (CD), 48000 (video/DAW), 88200, 96000, 192000 (studio). Any integer 3000-384000 is accepted."),
        )
        .param(
            Param::enumv("format", ["wav", "flac", "mp3", "ogg", "m4a"])
                .default("wav")
                .describe("Output format (default wav). wav/flac are lossless (best for a clean resample); mp3/ogg/m4a are lossy and encode at 192 kbps."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioResampler;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-resampler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change an audio file's sample rate (Hz) with high-quality resampling",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Change an audio file's sample rate (Hz) using ffmpeg's high-quality windowed-sinc resampler. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call) plus rate (target Hz, e.g. 44100 or 48000; any integer 3000-384000). format selects the output container: wav (default) and flac are lossless, best for a clean resample; mp3/ogg/m4a are lossy and encode at 192 kbps. Upsampling never adds detail the source lacks — it just changes the rate. Any input ffmpeg can decode works; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioResampler {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; rate/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-resampler")?;
    let format = args.format.as_deref().unwrap_or(DEFAULT_FORMAT);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates rate, parses format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_resample(&ffmpeg_in, args.rate, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; filename keeps the original
    //    stem with a rate suffix + the new extension (song.wav → song-16000hz.flac).
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, &format!("-{}hz", args.rate), fmt.ext());
    let for_llm = format!(
        "resampled {in_filename} to {} Hz ({}, {output_size} bytes)",
        args.rate,
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
    /// wording), so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "rate":   { "type": "integer", "minimum": 3000, "maximum": 384000, "description": "Target sample rate in Hz. Common values: 8000/16000 (speech), 22050, 32000, 44100 (CD), 48000 (video/DAW), 88200, 96000, 192000 (studio). Any integer 3000-384000 is accepted." },
                    "format": { "type": "string", "enum": ["wav", "flac", "mp3", "ogg", "m4a"], "default": "wav", "description": "Output format (default wav). wav/flac are lossless (best for a clean resample); mp3/ogg/m4a are lossy and encode at 192 kbps." }
                },
                "additionalProperties": false,
                "required": ["rate"],
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
    fn output_filename_adds_rate_suffix_and_swaps_extension() {
        assert_eq!(
            filename_with_suffix("song.wav", "-16000hz", "flac"),
            "song-16000hz.flac"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-8000hz", "mp3"),
            "voice memo-8000hz.mp3"
        );
    }
}
