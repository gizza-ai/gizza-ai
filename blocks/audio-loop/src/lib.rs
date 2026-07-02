//! gizza-ai/audio-loop — fetch an audio URL or attachment ref and repeat it
//! to a target duration or a number of plays. Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Mode/bounds validation
//! and the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_loop_core::{fmt_num, parse_format, plan_loop, DEFAULT_DURATION_S};
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
    duration: Option<f64>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("duration")
                .min(0.0)
                .max(3600.0)
                .default(30.0)
                .describe("Target output length in seconds (default 30, max 3600). The clip repeats and is cut to exactly this length; takes precedence when > 0. Set 0 to use count instead."),
        )
        .param(
            Param::integer("count")
                .min(0.0)
                .max(100.0)
                .default(0)
                .describe("Total number of plays (2-100), used when duration is 0 — e.g. 3 plays the clip three times back-to-back with no cut at the end."),
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
struct AudioLoop;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-loop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Loop a sound to a target duration or number of plays",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Repeat a short audio clip seamlessly. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). duration (seconds, default 30, max 3600) loops the clip and cuts the output to exactly that length — the usual way to turn a short sound into background audio; or set duration to 0 and count (2-100) to play the clip a whole number of times instead. The audio is re-encoded, so joins are sample-level (no container-copy clicks). Output format mp3 (192 kbps, default), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioLoop {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; mode/bounds validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-loop")?;
    let duration = args.duration.unwrap_or(DEFAULT_DURATION_S);
    let count = args.count.unwrap_or(0);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — picks the mode + validates).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_loop(&ffmpeg_in, duration, count, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; say which mode ran.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-loop", fmt.ext());
    let what = if duration > 0.0 {
        format!("to {} s", fmt_num(duration))
    } else {
        format!("{count}×")
    };
    let for_llm = format!(
        "looped {in_filename} {what} ({output_size} bytes {})",
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
    /// wording), so the expected JSON uses that shared wording. The number
    /// param's default serializes as a float (`30.0`), the integer's as an
    /// integer (`0`); whole-number bounds render as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "duration": { "type": "number", "minimum": 0, "maximum": 3600, "default": 30.0, "description": "Target output length in seconds (default 30, max 3600). The clip repeats and is cut to exactly this length; takes precedence when > 0. Set 0 to use count instead." },
                    "count":    { "type": "integer", "minimum": 0, "maximum": 100, "default": 0, "description": "Total number of plays (2-100), used when duration is 0 — e.g. 3 plays the clip three times back-to-back with no cut at the end." },
                    "format":   { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_loop_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("rain.wav", "-loop", "mp3"),
            "rain-loop.mp3"
        );
        assert_eq!(
            filename_with_suffix("white noise.m4a", "-loop", "flac"),
            "white noise-loop.flac"
        );
    }
}
