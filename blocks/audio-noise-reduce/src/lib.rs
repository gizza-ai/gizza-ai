//! gizza-ai/audio-noise-reduce — fetch an audio URL or attachment ref and reduce
//! steady background hiss/hum with ffmpeg's `afftdn`/`anlmdn` denoiser (an
//! optional 80 Hz high-pass cuts low-frequency hum). Part of the audio-input
//! family (`Input::Audio`). The audio is re-encoded to the chosen format
//! (denoising rewrites samples, so a lossless copy is impossible).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Strength/method/format
//! validation and the pure argv builder live in `core`, shared with the page.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_noise_reduce_core::{parse_format, plan, DEFAULT_STRENGTH};
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
    strength: Option<f64>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    remove_hum: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("strength")
                .default(12.0)
                .min(1.0)
                .max(100.0)
                .describe("How aggressively to reduce noise, 1–100 (higher removes more but risks a hollow/robotic sound). Start around 12 and raise gradually."),
        )
        .param(
            Param::enumv("method", ["afftdn", "anlmdn"])
                .default("afftdn")
                .describe("Denoiser: afftdn (FFT-based, fast, default) or anlmdn (non-local means, slower, gentler on transients)."),
        )
        .param(
            Param::boolean("remove_hum")
                .default(false)
                .describe("Also cut low-frequency hum/rumble below 80 Hz (mains buzz, HVAC) with a high-pass filter. Default off."),
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
struct AudioNoiseReduce;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-noise-reduce",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce background hiss/hum in an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Reduce steady background hiss/hum/noise in an audio file using ffmpeg's afftdn (FFT) or anlmdn (non-local means) denoiser. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). strength is 1–100 (higher removes more but risks a hollow sound; start around 12). method is afftdn (fast, default) or anlmdn (slower, gentler). Set remove_hum to also cut low-frequency hum below 80 Hz. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl AudioNoiseReduce {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; strength/method/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-noise-reduce")?;
    let strength = args.strength.unwrap_or(DEFAULT_STRENGTH);
    let method = args.method.as_deref().unwrap_or("afftdn");
    let remove_hum = args.remove_hum.unwrap_or(false);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates strength/method/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, strength, method, remove_hum, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-denoised", fmt.ext());
    let for_llm = format!(
        "reduced noise in {in_filename} (strength {strength}, {method}) → {output_size} bytes {}",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. Note the number
    /// param's default serializes as `12.0` (float), per the documented
    /// drift-guard gotcha.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "strength":   { "type": "number", "minimum": 1, "maximum": 100, "default": 12.0, "description": "How aggressively to reduce noise, 1–100 (higher removes more but risks a hollow/robotic sound). Start around 12 and raise gradually." },
                    "method":     { "type": "string", "enum": ["afftdn", "anlmdn"], "default": "afftdn", "description": "Denoiser: afftdn (FFT-based, fast, default) or anlmdn (non-local means, slower, gentler on transients)." },
                    "remove_hum": { "type": "boolean", "default": false, "description": "Also cut low-frequency hum/rumble below 80 Hz (mains buzz, HVAC) with a high-pass filter. Default off." },
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
    fn output_filename_uses_denoised_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("interview.wav", "-denoised", "mp3"),
            "interview-denoised.mp3"
        );
    }
}
