//! gizza-ai/audio-pad-silence — fetch an audio URL or attachment ref and add a
//! chosen length of silence to the start and/or the end of the clip. Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Bounds validation and the
//! pure argv builder live in `core`, shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_pad_silence_core::{
    fmt_num, parse_format, plan_pad, DEFAULT_END_S, DEFAULT_START_S,
};
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
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("start")
                .min(0.0)
                .max(3600.0)
                .default(2.0)
                .describe("Seconds of silence to add BEFORE the clip (default 2, max 3600). Decimals are allowed down to 0.001 (1 ms) — 0.5 is half a second. Use 0 for no lead-in. Example: 2 gives a two-second run-up before the audio starts."),
        )
        .param(
            Param::number("end")
                .min(0.0)
                .max(3600.0)
                .default(0.0)
                .describe("Seconds of silence to add AFTER the clip (default 0, max 3600). Decimals are allowed — 1.5 is a second and a half. Set start and end together to pad both ends in one pass; at least one of them must be greater than 0."),
        )
        .param(
            Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"])
                .default("mp3")
                .describe("Output audio format. Default mp3 (192 kbps); wav is uncompressed PCM, flac is lossless, ogg is 192 kbps Vorbis, m4a is AAC."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioPadSilence;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-pad-silence",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add silence to the start and/or end of an audio clip",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add a chosen length of silence to the start and/or the end of an audio clip. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call) plus start and/or end in seconds (start defaults to 2, end to 0; max 3600 each, decimals allowed down to 0.001). Both sides can be padded in one pass; at least one must be greater than 0. The clip itself is unchanged and the output is exactly start + original + end seconds long — useful for IVR prompts, ad slots, podcast intros and alignment. The audio is re-encoded so the silence joins cleanly (no container-copy clicks). Output format mp3 (192 kbps, default), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioPadSilence {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; bounds validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-pad-silence")?;
    let start = args.start.unwrap_or(DEFAULT_START_S);
    let end = args.end.unwrap_or(DEFAULT_END_S);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates + builds the chain).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_pad(&ffmpeg_in, start, end, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-padded", fmt.ext());
    let for_llm = format!(
        "padded {in_filename} with {} s of silence before and {} s after ({output_size} bytes {})",
        fmt_num(start),
        fmt_num(end),
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
    /// wording). Number-param defaults serialize as floats (`2.0`, `0.0`);
    /// whole-number bounds render as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "start":  { "type": "number", "minimum": 0, "maximum": 3600, "default": 2.0, "description": "Seconds of silence to add BEFORE the clip (default 2, max 3600). Decimals are allowed down to 0.001 (1 ms) — 0.5 is half a second. Use 0 for no lead-in. Example: 2 gives a two-second run-up before the audio starts." },
                    "end":    { "type": "number", "minimum": 0, "maximum": 3600, "default": 0.0, "description": "Seconds of silence to add AFTER the clip (default 0, max 3600). Decimals are allowed — 1.5 is a second and a half. Set start and end together to pad both ends in one pass; at least one of them must be greater than 0." },
                    "format": { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps); wav is uncompressed PCM, flac is lossless, ogg is 192 kbps Vorbis, m4a is AAC." }
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
    fn output_filename_uses_padded_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("intro.wav", "-padded", "mp3"),
            "intro-padded.mp3"
        );
        assert_eq!(
            filename_with_suffix("ivr prompt.m4a", "-padded", "flac"),
            "ivr prompt-padded.flac"
        );
    }
}
