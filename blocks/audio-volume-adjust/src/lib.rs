//! gizza-ai/audio-volume-adjust — fetch an audio URL or attachment ref and
//! boost or cut its gain (dB or linear factor), with an optional 0 dBFS peak
//! limiter so boosts don't clip. Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Amount/unit validation and
//! the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_volume_adjust_core::{parse_format, plan_volume};
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
    amount: f64,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    limiter: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("amount")
                .required()
                .min(-60.0)
                .max(60.0)
                .describe("How much to change the volume: with unit=db, decibels (6 boosts, -6 cuts, 0 not allowed); with unit=factor, a multiplier in (0, 16] (2 doubles, 0.5 halves)."),
        )
        .param(
            Param::enumv("unit", ["db", "factor"])
                .default("db")
                .describe("How amount is interpreted: decibels (default) or a linear factor."),
        )
        .param(
            Param::boolean("limiter")
                .default(true)
                .describe("Cap peaks at 0 dBFS (alimiter) so boosts don't clip. Default on; disable for exact linear gain."),
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
struct AudioVolumeAdjust;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-volume-adjust",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Boost or cut audio volume by dB or a factor",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Increase or decrease an audio file's volume. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). amount is in decibels by default (6 boosts, -6 cuts; range ±60) or a linear factor with unit=factor (2 doubles, 0.5 halves; range (0,16]). A peak limiter (on by default) caps output at 0 dBFS so boosts don't clip. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a.",
        parameters = schema_json()
    ),
)]
impl AudioVolumeAdjust {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; amount/unit/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-volume-adjust")?;
    let unit = args.unit.as_deref().unwrap_or("db");
    let limiter = args.limiter.unwrap_or(true);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates amount for the unit).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_volume(&ffmpeg_in, args.amount, unit, limiter, format)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-volume", fmt.ext());
    let change = if unit == "factor" {
        format!("x{}", args.amount)
    } else {
        format!("{:+} dB", args.amount)
    };
    let for_llm = format!(
        "adjusted volume of {in_filename} by {change} ({output_size} bytes {})",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "amount":  { "type": "number", "minimum": -60, "maximum": 60, "description": "How much to change the volume: with unit=db, decibels (6 boosts, -6 cuts, 0 not allowed); with unit=factor, a multiplier in (0, 16] (2 doubles, 0.5 halves)." },
                    "unit":    { "type": "string", "enum": ["db", "factor"], "default": "db", "description": "How amount is interpreted: decibels (default) or a linear factor." },
                    "limiter": { "type": "boolean", "default": true, "description": "Cap peaks at 0 dBFS (alimiter) so boosts don't clip. Default on; disable for exact linear gain." },
                    "format":  { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
                },
                "additionalProperties": false,
                "required": ["amount"],
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
    fn output_filename_uses_volume_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.flac", "-volume", "mp3"),
            "song-volume.mp3"
        );
    }
}
