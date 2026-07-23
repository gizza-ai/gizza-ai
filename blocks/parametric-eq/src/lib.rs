//! gizza-ai/parametric-eq — fetch an audio URL or attachment ref and apply a
//! three-band PARAMETRIC equalizer: every band's centre frequency, gain and Q
//! (bandwidth) is user-set. Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Band validation and the
//! pure argv builder live in `core`, shared with the page.

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
use gizza_ai_parametric_eq_core::{parse_format, Band, DEFAULT_FREQS, DEFAULT_Q};
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
    band1_freq: Option<f64>,
    #[serde(default)]
    band1_gain: Option<f64>,
    #[serde(default)]
    band1_q: Option<f64>,
    #[serde(default)]
    band2_freq: Option<f64>,
    #[serde(default)]
    band2_gain: Option<f64>,
    #[serde(default)]
    band2_q: Option<f64>,
    #[serde(default)]
    band3_freq: Option<f64>,
    #[serde(default)]
    band3_gain: Option<f64>,
    #[serde(default)]
    band3_q: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("band1_freq")
                .min(20.0)
                .max(20000.0)
                .default(100.0)
                .describe("Band 1 centre frequency in Hz (20–20000). Default 100 (low end). Only used when band1_gain is non-zero."),
        )
        .param(
            Param::number("band1_gain")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Band 1 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off."),
        )
        .param(
            Param::number("band1_q")
                .min(0.1)
                .max(10.0)
                .default(1.0)
                .describe("Band 1 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1."),
        )
        .param(
            Param::number("band2_freq")
                .min(20.0)
                .max(20000.0)
                .default(1000.0)
                .describe("Band 2 centre frequency in Hz (20–20000). Default 1000 (mids). Only used when band2_gain is non-zero."),
        )
        .param(
            Param::number("band2_gain")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Band 2 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off."),
        )
        .param(
            Param::number("band2_q")
                .min(0.1)
                .max(10.0)
                .default(1.0)
                .describe("Band 2 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1."),
        )
        .param(
            Param::number("band3_freq")
                .min(20.0)
                .max(20000.0)
                .default(5000.0)
                .describe("Band 3 centre frequency in Hz (20–20000). Default 5000 (highs). Only used when band3_gain is non-zero."),
        )
        .param(
            Param::number("band3_gain")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Band 3 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off."),
        )
        .param(
            Param::number("band3_q")
                .min(0.1)
                .max(10.0)
                .default(1.0)
                .describe("Band 3 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1."),
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
struct ParametricEq;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parametric-eq",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply a 3-band parametric EQ with adjustable frequency, gain and Q",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a three-band parametric equalizer to an audio file. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Each band has its own centre frequency (band1_freq/band2_freq/band3_freq, 20–20000 Hz), gain (band1_gain/band2_gain/band3_gain, -20 to 20 dB) and Q/bandwidth (band1_q/band2_q/band3_q, 0.1–10). A band is applied only when its gain is non-zero (a low Q is a wide gentle bell, a high Q a narrow surgical one); at least one band must be non-zero. Defaults place the bands at 100 Hz, 1 kHz and 5 kHz. Typical fixes: band 2 at 3 kHz gain +4 for vocal presence, band 1 at 200 Hz gain -6 to cut mud, band 3 at 8 kHz gain +3 for air. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl ParametricEq {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_parametric_eq_core::{fmt_num, plan_eq};

    // 1. Parse args; band/format validation lives in core's plan. Unset fields
    //    fall back to sensible defaults (bands off, spread across the spectrum).
    let args: Args = serde_json::from_slice(&body).invalid_args("parametric-eq")?;
    let bands = [
        Band::new(
            args.band1_freq.unwrap_or(DEFAULT_FREQS[0]),
            args.band1_gain.unwrap_or(0.0),
            args.band1_q.unwrap_or(DEFAULT_Q),
        ),
        Band::new(
            args.band2_freq.unwrap_or(DEFAULT_FREQS[1]),
            args.band2_gain.unwrap_or(0.0),
            args.band2_q.unwrap_or(DEFAULT_Q),
        ),
        Band::new(
            args.band3_freq.unwrap_or(DEFAULT_FREQS[2]),
            args.band3_gain.unwrap_or(0.0),
            args.band3_q.unwrap_or(DEFAULT_Q),
        ),
    ];
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates bands + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_eq(&ffmpeg_in, &bands, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied bands so
    //    the LLM can echo what changed. `{:+}` prints 4.0 as "+4".
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-eq", fmt.ext());
    let applied: Vec<String> = bands
        .iter()
        .filter(|b| b.gain != 0.0)
        .map(|b| format!("{} Hz {:+} dB (Q {})", fmt_num(b.freq), b.gain, fmt_num(b.q)))
        .collect();
    let for_llm = format!(
        "equalized {in_filename}: {} ({output_size} bytes {})",
        applied.join(", "),
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
    /// wording). Number-param defaults serialize as floats (`0.0`, `100.0`),
    /// whole-number bounds as integers (`20`, `20000`), fractional bounds as
    /// floats (`0.1`).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "band1_freq": { "type": "number", "minimum": 20, "maximum": 20000, "default": 100.0, "description": "Band 1 centre frequency in Hz (20–20000). Default 100 (low end). Only used when band1_gain is non-zero." },
                    "band1_gain": { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Band 1 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off." },
                    "band1_q":    { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "Band 1 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1." },
                    "band2_freq": { "type": "number", "minimum": 20, "maximum": 20000, "default": 1000.0, "description": "Band 2 centre frequency in Hz (20–20000). Default 1000 (mids). Only used when band2_gain is non-zero." },
                    "band2_gain": { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Band 2 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off." },
                    "band2_q":    { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "Band 2 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1." },
                    "band3_freq": { "type": "number", "minimum": 20, "maximum": 20000, "default": 5000.0, "description": "Band 3 centre frequency in Hz (20–20000). Default 5000 (highs). Only used when band3_gain is non-zero." },
                    "band3_gain": { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Band 3 gain in dB (-20 to 20). Positive boosts, negative cuts, 0 leaves the band off." },
                    "band3_q":    { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "Band 3 Q / bandwidth (0.1–10). Low Q = wide, gentle bell; high Q = narrow, surgical. Default 1." },
                    "format":     { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_eq_suffix_and_format_ext() {
        assert_eq!(filename_with_suffix("song.wav", "-eq", "mp3"), "song-eq.mp3");
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-eq", "flac"),
            "voice memo-eq.flac"
        );
    }
}
