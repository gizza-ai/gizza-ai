//! gizza-ai/audio-pitch-shift — fetch an audio URL or attachment ref, shift its
//! pitch up or down by semitones WITHOUT changing tempo/duration, and return
//! the re-encoded result as mp3/wav/ogg/flac/m4a.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Validation and the pure
//! argv builder (the aresample→asetrate→aresample→atempo chain) live in `core`,
//! shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_audio_pitch_shift_core::{parse_format, plan_pitch_shift};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    semitones: f64,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("semitones")
                .required()
                .min(-24.0)
                .max(24.0)
                .describe("How far to shift the pitch, in semitones (fractional values allowed, e.g. 0.5 = 50 cents). Positive raises the pitch, negative lowers it: 12 = one octave up, -12 = one octave down. Range -24 to 24; 0 is rejected as a no-op."),
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
struct AudioPitchShift;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-pitch-shift",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shift audio pitch by semitones without changing tempo",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Shift the pitch of an audio file up or down by semitones without changing its speed or duration (transpose a song to another key, deepen a voice, make a chipmunk effect). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). semitones may be fractional: 12 = one octave up, -12 = one octave down, range -24 to 24 (0 is rejected as a no-op). Tempo is preserved via a resample + atempo chain; output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a.",
        parameters = schema_json()
    ),
)]
impl AudioPitchShift {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; semitones/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-pitch-shift")?;
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates semitones/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_pitch_shift(&ffmpeg_in, args.semitones, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime + extension.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-pitch-shifted", fmt.ext());
    let direction = if args.semitones > 0.0 { "up" } else { "down" };
    let for_llm = format!(
        "pitch-shifted {in_filename} {direction} by {} semitones, tempo unchanged ({output_size} bytes {})",
        args.semitones.abs(),
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
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "semitones": { "type": "number", "minimum": -24, "maximum": 24, "description": "How far to shift the pitch, in semitones (fractional values allowed, e.g. 0.5 = 50 cents). Positive raises the pitch, negative lowers it: 12 = one octave up, -12 = one octave down. Range -24 to 24; 0 is rejected as a no-op." },
                    "format":    { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
                },
                "additionalProperties": false,
                "required": ["semitones"],
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
    fn output_filename_uses_pitch_shifted_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.ogg", "-pitch-shifted", "mp3"),
            "song-pitch-shifted.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice.mp3", "-pitch-shifted", "wav"),
            "voice-pitch-shifted.wav"
        );
    }
}
