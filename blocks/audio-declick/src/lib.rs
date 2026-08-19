//! gizza-ai/audio-declick — fetch an audio URL or attachment ref and repair
//! impulsive noise (vinyl clicks and pops, tape ticks, digital buffer glitches)
//! with ffmpeg's `adeclick`, which detects offending samples and replaces them
//! with values interpolated from their neighbours by an autoregressive model.
//! An optional `adeclip` stage does the same interpolation over clipped
//! (flat-topped) peaks. Part of the audio-input family (`Input::Audio`).
//!
//! Audio-only: `-vn` drops any attached picture, and the result is re-encoded to
//! the chosen container (repairing rewrites samples, so a stream copy is
//! impossible).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Option validation and the
//! pure argv builder live in `core`, shared verbatim with the page.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_declick_core::{
    parse_format, plan, DEFAULT_BURST, DEFAULT_STRENGTH, DEFAULT_WINDOW_MS,
};
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
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
    window_ms: Option<f64>,
    #[serde(default)]
    burst: Option<i64>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    declip: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("strength")
                .default(50.0)
                .min(1.0)
                .max(100.0)
                .describe("How aggressively to detect clicks, 1–100 (higher flags more samples as damage but can dull transients). Default 50, which repairs ordinary full-scale clicks."),
        )
        .param(
            Param::number("window_ms")
                .default(55.0)
                .min(10.0)
                .max(100.0)
                .describe("Length of each repair window in milliseconds, 10–100. Default 55. Raise it for wide vinyl thumps, lower it for short digital ticks."),
        )
        .param(
            Param::integer("burst")
                .default(2)
                .min(0.0)
                .max(10.0)
                .describe("How far apart two detections are still fused into one burst, 0–10 (percent of the window). Default 2; 0 repairs each click on its own."),
        )
        .param(
            Param::enumv("method", ["add", "save"])
                .default("add")
                .describe("Overlap method: add (default, smoothest, recomputes every sample) or save (only flagged samples are replaced, the rest passes through untouched)."),
        )
        .param(
            Param::boolean("declip")
                .default(false)
                .describe("Also interpolate over clipped (flat-topped) peaks with adeclip after declicking. Default off."),
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
struct AudioDeclick;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-declick",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove clicks, pops and crackle from an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Repair clicks, pops, ticks and vinyl crackle in an audio file using ffmpeg's adeclick, which replaces damaged samples with values interpolated from their neighbours. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). strength is 1–100 (higher detects more damage; default 50). window_ms is the 10–100 ms repair window (default 55; longer for wide thumps, shorter for digital ticks). burst is 0–10 (how far apart detections fuse into one burst; default 2). method is add (default, smoothest) or save (leaves undetected samples untouched). Set declip to also interpolate over clipped peaks. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl AudioDeclick {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; range/enum validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-declick")?;
    let strength = args.strength.unwrap_or(DEFAULT_STRENGTH);
    let window_ms = args.window_ms.unwrap_or(DEFAULT_WINDOW_MS);
    let burst = args.burst.unwrap_or(DEFAULT_BURST);
    let method = args.method.as_deref().unwrap_or("add");
    let declip = args.declip.unwrap_or(false);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates every option).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in, strength, window_ms, burst, method, declip, format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-declicked", fmt.ext());
    let declip_note = if declip { " + declip" } else { "" };
    let for_llm = format!(
        "declicked {in_filename} (strength {strength}, window {window_ms} ms, burst {burst}, method {method}{declip_note}) → {output_size} bytes {}",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. Note the number
    /// params' defaults serialize as floats (`50.0`) while the integer param's
    /// stays `2`, per the documented drift-guard gotcha.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "strength":  { "type": "number", "minimum": 1, "maximum": 100, "default": 50.0, "description": "How aggressively to detect clicks, 1–100 (higher flags more samples as damage but can dull transients). Default 50, which repairs ordinary full-scale clicks." },
                    "window_ms": { "type": "number", "minimum": 10, "maximum": 100, "default": 55.0, "description": "Length of each repair window in milliseconds, 10–100. Default 55. Raise it for wide vinyl thumps, lower it for short digital ticks." },
                    "burst":     { "type": "integer", "minimum": 0, "maximum": 10, "default": 2, "description": "How far apart two detections are still fused into one burst, 0–10 (percent of the window). Default 2; 0 repairs each click on its own." },
                    "method":    { "type": "string", "enum": ["add", "save"], "default": "add", "description": "Overlap method: add (default, smoothest, recomputes every sample) or save (only flagged samples are replaced, the rest passes through untouched)." },
                    "declip":    { "type": "boolean", "default": false, "description": "Also interpolate over clipped (flat-topped) peaks with adeclip after declicking. Default off." },
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

    /// The envelope's mime/extension must follow the chosen output format, not
    /// the input's — repairing always re-encodes.
    #[test]
    fn output_filename_and_mime_follow_the_chosen_format() {
        assert_eq!(
            filename_with_suffix("vinyl-rip.wav", "-declicked", "mp3"),
            "vinyl-rip-declicked.mp3"
        );
        assert_eq!(parse_format("flac").unwrap().mime(), "audio/flac");
        assert_eq!(parse_format("flac").unwrap().ext(), "flac");
    }

    /// The descriptor's advertised defaults must be the ones `run()` falls back
    /// to when a param is omitted.
    #[test]
    fn descriptor_defaults_match_core_defaults() {
        assert_eq!(DEFAULT_STRENGTH, 50.0);
        assert_eq!(DEFAULT_WINDOW_MS, 55.0);
        assert_eq!(DEFAULT_BURST, 2);
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(props["strength"]["default"], 50.0);
        assert_eq!(props["window_ms"]["default"], 55.0);
        assert_eq!(props["burst"]["default"], 2);
    }
}
