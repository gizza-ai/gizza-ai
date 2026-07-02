//! gizza-ai/trim-audio — fetch an audio URL or attachment ref, keep or remove a
//! `[start, end]` selection (optional edge fades), and return the re-encoded
//! result as mp3/wav/flac/m4a. First tool of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Validation and the pure
//! argv builder live in `core`, shared with the page.

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
use gizza_ai_trim_audio_core::{parse_format, plan_trim_audio};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    start: f64,
    end: f64,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    fade: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("start")
                .required()
                .min(0.0)
                .describe("Start of the selection in seconds."),
        )
        .param(
            Param::number("end")
                .required()
                .min(0.0)
                .describe("End of the selection in seconds; must be greater than start."),
        )
        .param(
            Param::enumv("mode", ["keep", "remove"])
                .default("keep")
                .describe("keep extracts just the [start, end] selection; remove deletes that range and joins the rest. Default keep."),
        )
        .param(
            Param::enumv("format", ["mp3", "wav", "flac", "m4a"])
                .default("mp3")
                .describe("Output audio format. Default mp3 (192 kbps)."),
        )
        .param(
            Param::boolean("fade")
                .default(false)
                .describe("Apply a short fade-in/out (up to 0.5s) at the selection edges to avoid clicks. Keep mode only."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TrimAudio;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/trim-audio",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Keep or remove a [start, end] range of an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Trim an audio file: keep just the [start, end] selection (mode=keep) or delete that range and join the rest (mode=remove). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Times are in seconds. Output is re-encoded to mp3 (192 kbps), wav, flac or m4a; an optional short fade avoids clicks at the cut edges (keep mode only).",
        parameters = schema_json()
    ),
)]
impl TrimAudio {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; range/mode/format/fade validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("trim-audio")?;
    let mode = args.mode.as_deref().unwrap_or("keep");
    let format = args.format.as_deref().unwrap_or("mp3");
    let fade = args.fade.unwrap_or(false);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates start/end/mode/format/fade).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_trim_audio(&ffmpeg_in, args.start, args.end, mode, format, fade)
            .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime + extension.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-trimmed", fmt.ext());
    let action = if mode == "remove" { "removed" } else { "kept" };
    let for_llm = format!(
        "{action} [{}s, {}s] of {in_filename} ({output_size} bytes {})",
        args.start,
        args.end,
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
                    "start":  { "type": "number", "minimum": 0, "description": "Start of the selection in seconds." },
                    "end":    { "type": "number", "minimum": 0, "description": "End of the selection in seconds; must be greater than start." },
                    "mode":   { "type": "string", "enum": ["keep", "remove"], "default": "keep", "description": "keep extracts just the [start, end] selection; remove deletes that range and joins the rest. Default keep." },
                    "format": { "type": "string", "enum": ["mp3", "wav", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." },
                    "fade":   { "type": "boolean", "default": false, "description": "Apply a short fade-in/out (up to 0.5s) at the selection edges to avoid clicks. Keep mode only." }
                },
                "additionalProperties": false,
                "required": ["start", "end"],
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
    fn output_filename_uses_trimmed_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.ogg", "-trimmed", "mp3"),
            "song-trimmed.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice.mp3", "-trimmed", "wav"),
            "voice-trimmed.wav"
        );
    }
}
