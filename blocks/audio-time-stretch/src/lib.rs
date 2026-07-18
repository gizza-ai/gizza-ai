//! gizza-ai/audio-time-stretch — fetch an audio URL or attachment ref, speed it
//! up or slow it down by a playback factor WITHOUT changing the pitch (a "time
//! stretch"), and return the re-encoded result as mp3/wav/ogg/flac/m4a.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Validation and the pure
//! argv builder (the `atempo` WSOLA chain) live in `core`, shared with the page.

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
use gizza_ai_audio_time_stretch_core::{parse_format, plan_time_stretch};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    factor: f64,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("factor")
                .required()
                .min(0.25)
                .max(4.0)
                .describe("Playback-speed multiplier (tempo/BPM factor). 2 = twice as fast (half the duration), 0.5 = half speed (double the duration), 1.5 = 50% faster. As a percentage, 150% = 1.5; as BPM, factor = target BPM / source BPM. Range 0.25 to 4; 1 is rejected as a no-op. Pitch is preserved."),
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
struct AudioTimeStretch;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-time-stretch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Speed audio up or down without changing the pitch",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Speed an audio file up or slow it down by a playback factor without changing its pitch (a time stretch): make a podcast play at 1.5x, slow a solo to half speed for transcription, or hit a target BPM. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). factor is the speed multiplier: 2 = twice as fast, 0.5 = half speed, 1.5 = 50% faster (150%); as BPM, factor = target BPM / source BPM. Range 0.25 to 4 (1 is rejected as a no-op). Pitch is preserved via ffmpeg's atempo WSOLA time-stretch; output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a.",
        parameters = schema_json()
    ),
)]
impl AudioTimeStretch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; factor/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-time-stretch")?;
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates factor/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_time_stretch(&ffmpeg_in, args.factor, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime + extension.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-time-stretched", fmt.ext());
    let direction = if args.factor > 1.0 { "faster" } else { "slower" };
    let for_llm = format!(
        "time-stretched {in_filename} to {}x ({direction}), pitch unchanged ({output_size} bytes {})",
        args.factor,
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
                    "factor": { "type": "number", "minimum": 0.25, "maximum": 4, "description": "Playback-speed multiplier (tempo/BPM factor). 2 = twice as fast (half the duration), 0.5 = half speed (double the duration), 1.5 = 50% faster. As a percentage, 150% = 1.5; as BPM, factor = target BPM / source BPM. Range 0.25 to 4; 1 is rejected as a no-op. Pitch is preserved." },
                    "format": { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
                },
                "additionalProperties": false,
                "required": ["factor"],
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
    fn output_filename_uses_time_stretched_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("podcast.ogg", "-time-stretched", "mp3"),
            "podcast-time-stretched.mp3"
        );
        assert_eq!(
            filename_with_suffix("solo.mp3", "-time-stretched", "wav"),
            "solo-time-stretched.wav"
        );
    }
}
