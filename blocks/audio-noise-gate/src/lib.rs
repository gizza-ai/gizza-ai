//! gizza-ai/audio-noise-gate — fetch an audio URL or attachment ref and apply a
//! downward noise gate with ffmpeg's `agate` filter (threshold, reduction/floor,
//! ratio, attack, release + rms/peak detection). Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Range validation and the
//! pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_noise_gate_core::{parse_format, plan_gate};
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
    threshold: Option<f64>,
    #[serde(default)]
    reduction: Option<f64>,
    #[serde(default)]
    ratio: Option<f64>,
    #[serde(default)]
    attack: Option<f64>,
    #[serde(default)]
    release: Option<f64>,
    #[serde(default)]
    detection: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("threshold")
                .min(-80.0)
                .max(0.0)
                .default(-35.0)
                .describe("Threshold in dB below full scale a signal must EXCEED to open the gate. Sound quieter than this is attenuated. Lower (e.g. -50) only gates the very quietest hiss; higher (e.g. -25) also clamps low speech/tails. Default -35."),
        )
        .param(
            Param::number("reduction")
                .min(0.0)
                .max(80.0)
                .default(30.0)
                .describe("How much quieter, in dB, the below-threshold signal gets while the gate is closed (the floor). 0 does nothing (rejected); 30 pushes background well down; 80 is near-silence. Default 30."),
        )
        .param(
            Param::number("ratio")
                .min(1.0)
                .max(20.0)
                .default(2.0)
                .describe("How steeply gain is pulled down below the threshold. 2 is a gentle gate, 10+ clamps hard toward the floor. 1 = barely gates. Default 2."),
        )
        .param(
            Param::number("attack")
                .min(0.01)
                .max(2000.0)
                .default(10.0)
                .describe("Attack time in milliseconds — how fast the gate opens once the level rises past the threshold. Fast (5) preserves word onsets; slow (50+) can clip the start of sounds. Default 10."),
        )
        .param(
            Param::number("release")
                .min(0.01)
                .max(9000.0)
                .default(250.0)
                .describe("Release time in milliseconds — how fast the gate closes once the level drops back below the threshold. Short (50) cuts tightly; long (500+) fades out smoothly and helps stand in for a hold time. Default 250."),
        )
        .param(
            Param::enumv("detection", ["rms", "peak"])
                .default("rms")
                .describe("How the gate measures level: rms tracks average loudness (smoother, ignores brief peaks), peak reacts to instantaneous sample peaks (catches sharp transients). Default rms."),
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
struct AudioNoiseGate;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-noise-gate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Gate audio below a threshold to silence background noise in quiet passages",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a downward noise gate to an audio file: sound below a threshold (background hiss, room tone, hum, breaths in the gaps) is pushed down or silenced while the wanted signal above the threshold passes untouched. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Controls: threshold (dB, -80..0, level a signal must exceed to open the gate), reduction (dB, 0..80, how much the closed gate attenuates — 0 is a no-op and is rejected), ratio (1..20, how steeply it clamps below the threshold), attack (ms, 0.01..2000, how fast it opens), release (ms, 0.01..9000, how fast it closes) and detection (rms|peak). This is a level-based dynamics gate, NOT spectral denoising (see audio-noise-reduce) and it does NOT shorten the file — the gaps stay in place, just quieter. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioNoiseGate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; control/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-noise-gate")?;
    let threshold = args.threshold.unwrap_or(-35.0);
    let reduction = args.reduction.unwrap_or(30.0);
    let ratio = args.ratio.unwrap_or(2.0);
    let attack = args.attack.unwrap_or(10.0);
    let release = args.release.unwrap_or(250.0);
    let detection = args.detection.as_deref().unwrap_or("rms");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates controls + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_gate(
        &ffmpeg_in, threshold, reduction, ratio, attack, release, detection, format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied settings
    //    so the LLM can echo what changed.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-gated", fmt.ext());
    let for_llm = format!(
        "gated {in_filename}: threshold {threshold} dB, reduction {reduction} dB, ratio {ratio}:1, attack {attack} ms, release {release} ms, {detection} detection ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats (`-35.0`),
    /// whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "number", "minimum": -80, "maximum": 0, "default": -35.0, "description": "Threshold in dB below full scale a signal must EXCEED to open the gate. Sound quieter than this is attenuated. Lower (e.g. -50) only gates the very quietest hiss; higher (e.g. -25) also clamps low speech/tails. Default -35." },
                    "reduction": { "type": "number", "minimum": 0, "maximum": 80, "default": 30.0, "description": "How much quieter, in dB, the below-threshold signal gets while the gate is closed (the floor). 0 does nothing (rejected); 30 pushes background well down; 80 is near-silence. Default 30." },
                    "ratio":     { "type": "number", "minimum": 1, "maximum": 20, "default": 2.0, "description": "How steeply gain is pulled down below the threshold. 2 is a gentle gate, 10+ clamps hard toward the floor. 1 = barely gates. Default 2." },
                    "attack":    { "type": "number", "minimum": 0.01, "maximum": 2000, "default": 10.0, "description": "Attack time in milliseconds — how fast the gate opens once the level rises past the threshold. Fast (5) preserves word onsets; slow (50+) can clip the start of sounds. Default 10." },
                    "release":   { "type": "number", "minimum": 0.01, "maximum": 9000, "default": 250.0, "description": "Release time in milliseconds — how fast the gate closes once the level drops back below the threshold. Short (50) cuts tightly; long (500+) fades out smoothly and helps stand in for a hold time. Default 250." },
                    "detection": { "type": "string", "enum": ["rms", "peak"], "default": "rms", "description": "How the gate measures level: rms tracks average loudness (smoother, ignores brief peaks), peak reacts to instantaneous sample peaks (catches sharp transients). Default rms." },
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
    fn output_filename_uses_gated_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("voice.wav", "-gated", "mp3"),
            "voice-gated.mp3"
        );
        assert_eq!(
            filename_with_suffix("interview take 2.m4a", "-gated", "flac"),
            "interview take 2-gated.flac"
        );
    }
}
