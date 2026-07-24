//! gizza-ai/audio-bleep-censor — fetch an audio URL or attachment ref and censor
//! one or more time regions: bleep them with a tone, silence them, or duck them
//! to a low level. Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Region parsing and the pure
//! argv builder (mute/duck `volume` gate + the bleep `amix` graph) live in
//! `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (a native registration
// call requiring ::new()). The supporting imports, constants, and Args type are
// only used inside the wasm32-gated impl, so they look "unused" natively.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_bleep_censor_core::{parse_format, parse_regions, plan, Mode, DEFAULT_TONE_HZ};
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    regions: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    tone_hz: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::string("regions").required().describe(
                "Time regions to censor, as a comma-separated list of start-end ranges. Each time is \
                 seconds (1.5) or mm:ss / hh:mm:ss (0:07, 1:02:03.5). Example: \"1.5-2.0, 0:07-0:08.5\". \
                 Up to 50 regions.",
            ),
        )
        .param(
            Param::enumv("mode", ["bleep", "mute", "duck"])
                .default("bleep")
                .describe(
                    "How each region is censored: bleep (mix a tone over it), mute (silence it), or \
                     duck (drop it to a quiet level). Default bleep.",
                ),
        )
        .param(
            Param::number("tone_hz")
                .min(100.0)
                .max(8000.0)
                .default(1000.0)
                .describe(
                    "Bleep tone frequency in Hz (100-8000, default 1000 = classic TV bleep). Only \
                     used when mode is bleep.",
                ),
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
struct AudioBleepCensor;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-bleep-censor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bleep, mute or duck time regions of an audio file to censor words",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Censor one or more time regions of an audio file. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). regions is a comma-separated list of start-end ranges (each time in seconds like 1.5, or mm:ss / hh:mm:ss like 0:07-0:08.5); up to 50 ranges. mode is bleep (default; mixes a tone over the region), mute (silences it) or duck (drops it to a quiet level). tone_hz sets the bleep frequency (100-8000 Hz, default 1000; bleep only). Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioBleepCensor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; region/mode/tone/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-bleep-censor")?;
    let mode = args.mode.as_deref().unwrap_or("bleep");
    let tone_hz = args.tone_hz.unwrap_or(DEFAULT_TONE_HZ);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates regions + tone + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, &args.regions, mode, tone_hz, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out what was censored.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let region_count = parse_regions(&args.regions)
        .map(|r| r.len())
        .map_err(SkillError::InvalidArgs)?;
    let action = match Mode::parse(mode).map_err(SkillError::InvalidArgs)? {
        Mode::Bleep => "bleeped",
        Mode::Mute => "muted",
        Mode::Duck => "ducked",
    };
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-censored", fmt.ext());
    let plural = if region_count == 1 { "region" } else { "regions" };
    let for_llm = format!(
        "{action} {region_count} {plural} of {in_filename} ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats; whole-number bounds
    /// as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "regions": { "type": "string", "description": "Time regions to censor, as a comma-separated list of start-end ranges. Each time is seconds (1.5) or mm:ss / hh:mm:ss (0:07, 1:02:03.5). Example: \"1.5-2.0, 0:07-0:08.5\". Up to 50 regions." },
                    "mode":    { "type": "string", "enum": ["bleep", "mute", "duck"], "default": "bleep", "description": "How each region is censored: bleep (mix a tone over it), mute (silence it), or duck (drop it to a quiet level). Default bleep." },
                    "tone_hz": { "type": "number", "minimum": 100, "maximum": 8000, "default": 1000.0, "description": "Bleep tone frequency in Hz (100-8000, default 1000 = classic TV bleep). Only used when mode is bleep." },
                    "format":  { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
                },
                "additionalProperties": false,
                "required": ["regions"],
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
    fn output_filename_uses_censored_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("podcast.wav", "-censored", "mp3"),
            "podcast-censored.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-censored", "ogg"),
            "voice memo-censored.ogg"
        );
    }
}
