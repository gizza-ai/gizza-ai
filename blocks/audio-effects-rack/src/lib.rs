//! gizza-ai/audio-effects-rack — fetch an audio URL or attachment ref and run
//! it through a five-stage effects rack (compression → chorus → tremolo →
//! echo → reverb) in one ffmpeg pass. Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. All effect validation and
//! the pure argv builder live in `core`, shared verbatim with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard test can exercise
// them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_effects_rack_core::{describe_stages, parse_format, plan_effects};
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
    reverb: Option<String>,
    #[serde(default)]
    echo: Option<f64>,
    #[serde(default)]
    chorus: Option<String>,
    #[serde(default)]
    tremolo: Option<f64>,
    #[serde(default)]
    compression: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("reverb", ["none", "room", "hall", "plate"])
                .default("none")
                .describe("Reverb space (multi-tap ambience): room is a tight small space, hall a big spacious tail, plate a bright dense sheen. none skips the reverb stage."),
        )
        .param(
            Param::number("echo")
                .min(0.0)
                .max(1000.0)
                .default(0.0)
                .describe("Echo delay in milliseconds (single repeat): 250 is a quarter-second slapback, 500 a long doubling. 0 turns echo off. Max 1000 ms."),
        )
        .param(
            Param::enumv("chorus", ["none", "light", "deep"])
                .default("none")
                .describe("Chorus (detuned doubling for width): light adds one gentle voice, deep two for a lush shimmer. none skips the chorus stage."),
        )
        .param(
            Param::number("tremolo")
                .min(0.0)
                .max(20.0)
                .default(0.0)
                .describe("Tremolo rate in Hz (amplitude wobble, 70% depth): 4 is a slow pulse, 8 a fast shimmer. 0 turns tremolo off. Range 0.1–20 Hz."),
        )
        .param(
            Param::enumv("compression", ["none", "light", "medium", "heavy"])
                .default("none")
                .describe("Dynamic-range compression (evens loud/quiet swings): light gently, heavy squashes hard for a loud, upfront sound. none skips it. This is loudness, not file-size, compression."),
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
struct AudioEffectsRack;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-effects-rack",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply reverb, echo, chorus, tremolo and compression to audio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Run an audio file through a five-stage effects rack in one pass. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Stages, applied in signal order: compression (none|light|medium|heavy — evens loudness), chorus (none|light|deep — detuned width), tremolo (0–20 Hz amplitude wobble, 0 = off), echo (0–1000 ms single repeat, 0 = off), reverb (none|room|hall|plate — ambience). Each stage left at none/0 is skipped; at least one must be active. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioEffectsRack {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; all effect/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-effects-rack")?;
    let reverb = args.reverb.as_deref().unwrap_or("none");
    let echo = args.echo.unwrap_or(0.0);
    let chorus = args.chorus.as_deref().unwrap_or("none");
    let tremolo = args.tremolo.unwrap_or(0.0);
    let compression = args.compression.as_deref().unwrap_or("none");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates every stage + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_effects(&ffmpeg_in, reverb, echo, chorus, tremolo, compression, format)
            .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the active stages so
    //    the LLM can echo exactly what was applied.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fx", fmt.ext());
    let stages = describe_stages(reverb, echo, chorus, tremolo, compression);
    let for_llm = format!(
        "audio effects on {in_filename}: {stages} ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats (`0.0`), whole-number
    /// bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "reverb":      { "type": "string", "enum": ["none", "room", "hall", "plate"], "default": "none", "description": "Reverb space (multi-tap ambience): room is a tight small space, hall a big spacious tail, plate a bright dense sheen. none skips the reverb stage." },
                    "echo":        { "type": "number", "minimum": 0, "maximum": 1000, "default": 0.0, "description": "Echo delay in milliseconds (single repeat): 250 is a quarter-second slapback, 500 a long doubling. 0 turns echo off. Max 1000 ms." },
                    "chorus":      { "type": "string", "enum": ["none", "light", "deep"], "default": "none", "description": "Chorus (detuned doubling for width): light adds one gentle voice, deep two for a lush shimmer. none skips the chorus stage." },
                    "tremolo":     { "type": "number", "minimum": 0, "maximum": 20, "default": 0.0, "description": "Tremolo rate in Hz (amplitude wobble, 70% depth): 4 is a slow pulse, 8 a fast shimmer. 0 turns tremolo off. Range 0.1–20 Hz." },
                    "compression": { "type": "string", "enum": ["none", "light", "medium", "heavy"], "default": "none", "description": "Dynamic-range compression (evens loud/quiet swings): light gently, heavy squashes hard for a loud, upfront sound. none skips it. This is loudness, not file-size, compression." },
                    "format":      { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_fx_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.wav", "-fx", "mp3"),
            "song-fx.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-fx", "flac"),
            "voice memo-fx.flac"
        );
    }
}
