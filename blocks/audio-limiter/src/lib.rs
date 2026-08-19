//! gizza-ai/audio-limiter — fetch an audio URL or attachment ref and apply a
//! brick-wall peak limiter with ffmpeg's `alimiter` (lookahead) filter: optional
//! input gain, a hard ceiling in dBFS, plus attack/release timing. Part of the
//! audio-input family (`Input::Audio`).
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

use gizza_ai_audio_limiter_core::{parse_format, plan_limit};
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
    ceiling: Option<f64>,
    #[serde(default)]
    gain: Option<f64>,
    #[serde(default)]
    attack: Option<f64>,
    #[serde(default)]
    release: Option<f64>,
    #[serde(default)]
    smooth_release: Option<bool>,
    #[serde(default)]
    auto_level: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("ceiling")
                .min(-24.0)
                .max(0.0)
                .default(-1.0)
                .describe("Brick-wall ceiling in dBFS — no output sample is allowed above it. -1 is the usual safety margin before lossy encoding, -0.3 squeezes out the last bit of level, -3 leaves generous headroom. Default -1."),
        )
        .param(
            Param::number("gain")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Input gain (drive) in dB applied BEFORE the ceiling. 0 only catches peaks that already exceed the ceiling; positive values push more of the signal into the limiter to make it louder. Default 0."),
        )
        .param(
            Param::number("attack")
                .min(0.1)
                .max(80.0)
                .default(5.0)
                .describe("Lookahead attack time in milliseconds — how fast the limiter clamps an incoming peak. Short (1-5) is transparent on speech, longer (20+) is smoother but lets more through. Default 5."),
        )
        .param(
            Param::number("release")
                .min(1.0)
                .max(8000.0)
                .default(50.0)
                .describe("Release time in milliseconds — how fast gain recovers after a peak. Short (20-50) is loud and tight, long (200+) is smoother and less pumpy. Default 50."),
        )
        .param(
            Param::boolean("smooth_release")
                .default(false)
                .describe("Average the release over recent gain reduction (ffmpeg's ASC) so dense, peaky material sounds less pumpy. Default false."),
        )
        .param(
            Param::boolean("auto_level")
                .default(false)
                .describe("After limiting, re-normalize the signal back up to full scale (loudness-maximizer behaviour). This deliberately overrides the ceiling, so leave it off when the ceiling must be honoured. Default false."),
        )
        .param(
            Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"])
                .default("mp3")
                .describe("Output audio format. Default mp3 (192 kbps); wav/flac are lossless and hold the ceiling exactly."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioLimiter;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-limiter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a brick-wall peak limiter so audio never clips above a chosen ceiling",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a brick-wall lookahead peak limiter to an audio file so no sample crosses a chosen ceiling. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Controls: ceiling (dBFS, -24..0, the hard wall; -1 is the usual safety margin), gain (dB, -20..20, drive applied before the ceiling to make the result louder), attack (ms, 0.1..80, how fast peaks are clamped), release (ms, 1..8000, how fast gain recovers), smooth_release (average the release over recent gain reduction) and auto_level (re-normalize back to full scale afterwards, which overrides the ceiling). This is peak limiting, not loudness normalization (audio-normalize) and not ratio-based dynamic-range compression (audio-compressor). A 0 dB ceiling with 0 dB gain and no auto-level is a no-op and is rejected. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; wav and flac hold the ceiling exactly, lossy formats can overshoot it slightly. Embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioLimiter {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-limiter")?;
    let ceiling = args.ceiling.unwrap_or(-1.0);
    let gain = args.gain.unwrap_or(0.0);
    let attack = args.attack.unwrap_or(5.0);
    let release = args.release.unwrap_or(50.0);
    let smooth_release = args.smooth_release.unwrap_or(false);
    let auto_level = args.auto_level.unwrap_or(false);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates controls + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_limit(
        &ffmpeg_in,
        ceiling,
        gain,
        attack,
        release,
        smooth_release,
        auto_level,
        format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied settings
    //    so the LLM can echo what changed. `{:+}` prints 6.0 as "+6".
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-limited", fmt.ext());
    let gain_note = if gain != 0.0 {
        format!(", gain {gain:+} dB")
    } else {
        String::new()
    };
    let auto_note = if auto_level {
        ", auto-levelled back to full scale"
    } else {
        ""
    };
    let for_llm = format!(
        "limited {in_filename}: ceiling {ceiling} dBFS{gain_note}, attack {attack} ms, release {release} ms{auto_note} ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats (`-1.0`),
    /// whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":            { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":            { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "ceiling":        { "type": "number", "minimum": -24, "maximum": 0, "default": -1.0, "description": "Brick-wall ceiling in dBFS — no output sample is allowed above it. -1 is the usual safety margin before lossy encoding, -0.3 squeezes out the last bit of level, -3 leaves generous headroom. Default -1." },
                    "gain":           { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Input gain (drive) in dB applied BEFORE the ceiling. 0 only catches peaks that already exceed the ceiling; positive values push more of the signal into the limiter to make it louder. Default 0." },
                    "attack":         { "type": "number", "minimum": 0.1, "maximum": 80, "default": 5.0, "description": "Lookahead attack time in milliseconds — how fast the limiter clamps an incoming peak. Short (1-5) is transparent on speech, longer (20+) is smoother but lets more through. Default 5." },
                    "release":        { "type": "number", "minimum": 1, "maximum": 8000, "default": 50.0, "description": "Release time in milliseconds — how fast gain recovers after a peak. Short (20-50) is loud and tight, long (200+) is smoother and less pumpy. Default 50." },
                    "smooth_release": { "type": "boolean", "default": false, "description": "Average the release over recent gain reduction (ffmpeg's ASC) so dense, peaky material sounds less pumpy. Default false." },
                    "auto_level":     { "type": "boolean", "default": false, "description": "After limiting, re-normalize the signal back up to full scale (loudness-maximizer behaviour). This deliberately overrides the ceiling, so leave it off when the ceiling must be honoured. Default false." },
                    "format":         { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps); wav/flac are lossless and hold the ceiling exactly." }
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
    fn output_filename_uses_limited_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("master.wav", "-limited", "mp3"),
            "master-limited.mp3"
        );
        assert_eq!(
            filename_with_suffix("podcast episode.m4a", "-limited", "flac"),
            "podcast episode-limited.flac"
        );
    }
}
