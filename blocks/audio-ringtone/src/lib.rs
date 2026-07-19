//! gizza-ai/audio-ringtone — cut a slice of a song into a phone ringtone
//! (m4r for iPhone, mp3 for Android), with optional loudness normalization
//! and edge fades. Audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared
//! shape across chat + CLI + page); the handler delegates source-resolution,
//! ffmpeg dispatch, and envelope-building to `block_utils`. Validation and
//! the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard + unit tests
// below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_audio_ringtone_core::{
    fmt_num, is_default_end, parse_format, plan_ringtone, resolve_end, DEFAULT_FADE_S,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    fade_in: Option<f64>,
    #[serde(default)]
    fade_out: Option<f64>,
    #[serde(default)]
    normalize: Option<bool>,
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
                .default(0.0)
                .describe("Where the ringtone starts in the source, in seconds (e.g. 45.5 to begin at the chorus). Default 0 = the beginning."),
        )
        .param(
            Param::number("end")
                .min(0.0)
                .describe("Where it ends, in seconds. 0 or omitted means start + 30 (a standard 30-second ringtone). The slice end - start must be 1-40 seconds; 40 s is iPhone's ringtone cap."),
        )
        .param(
            Param::number("fade_in")
                .min(0.0)
                .max(5.0)
                .default(0.5)
                .describe("Fade-in length in seconds (0-5). Default 0.5 — a short ramp that avoids a click at the cut. 0 disables."),
        )
        .param(
            Param::number("fade_out")
                .min(0.0)
                .max(5.0)
                .default(0.5)
                .describe("Fade-out length in seconds (0-5). Default 0.5 so the ringtone doesn't end abruptly. 0 disables."),
        )
        .param(
            Param::boolean("normalize")
                .default(true)
                .describe("Normalize loudness to -14 LUFS (EBU R128) so the ringtone rings loud and consistent on a phone speaker. Default true; disable to keep the source's original level."),
        )
        .param(
            Param::enumv("format", ["m4r", "mp3"])
                .default("m4r")
                .describe("m4r is the AAC format iPhone requires for ringtones; mp3 works on Android and most other phones. Both encode at 192 kbps. Default m4r."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioRingtone;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-ringtone",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Cut a slice of a song into a phone ringtone (m4r for iPhone, mp3 for Android)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Turn a song into a phone ringtone: cut the [start, end] slice (end 0/omitted = start + 30 seconds; the slice must be 1-40 s — 40 s is iPhone's ringtone cap), optionally normalize loudness to -14 LUFS so it rings loud on a phone speaker, add short edge fades (0.5 s by default), and export as m4r (the AAC format iPhone requires) or mp3 (Android and most other phones), both at 192 kbps. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Times are in seconds.",
        parameters = schema_json()
    ),
)]
impl AudioRingtone {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; range/fade/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-ringtone")?;
    let end = args.end.unwrap_or(0.0); // 0/omitted = start + 30 s
    let fade_in = args.fade_in.unwrap_or(DEFAULT_FADE_S);
    let fade_out = args.fade_out.unwrap_or(DEFAULT_FADE_S);
    let normalize = args.normalize.unwrap_or(true);
    let format = args.format.as_deref().unwrap_or("m4r");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates start/end/fades/format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_ringtone(
        &ffmpeg_in, args.start, end, fade_in, fade_out, normalize, format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime + extension.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-ringtone", fmt.ext());
    let end_desc = if is_default_end(end) {
        format!("{}s", fmt_num(resolve_end(args.start, end)))
    } else {
        format!("{}s", fmt_num(end))
    };
    let for_llm = format!(
        "cut [{}s, {end_desc}] of {in_filename} into a {} ringtone ({output_size} bytes{})",
        fmt_num(args.start),
        fmt.ext(),
        if normalize { ", normalized to -14 LUFS" } else { "" },
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
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "start":     { "type": "number", "minimum": 0, "default": 0.0, "description": "Where the ringtone starts in the source, in seconds (e.g. 45.5 to begin at the chorus). Default 0 = the beginning." },
                    "end":       { "type": "number", "minimum": 0, "description": "Where it ends, in seconds. 0 or omitted means start + 30 (a standard 30-second ringtone). The slice end - start must be 1-40 seconds; 40 s is iPhone's ringtone cap." },
                    "fade_in":   { "type": "number", "minimum": 0, "maximum": 5, "default": 0.5, "description": "Fade-in length in seconds (0-5). Default 0.5 — a short ramp that avoids a click at the cut. 0 disables." },
                    "fade_out":  { "type": "number", "minimum": 0, "maximum": 5, "default": 0.5, "description": "Fade-out length in seconds (0-5). Default 0.5 so the ringtone doesn't end abruptly. 0 disables." },
                    "normalize": { "type": "boolean", "default": true, "description": "Normalize loudness to -14 LUFS (EBU R128) so the ringtone rings loud and consistent on a phone speaker. Default true; disable to keep the source's original level." },
                    "format":    { "type": "string", "enum": ["m4r", "mp3"], "default": "m4r", "description": "m4r is the AAC format iPhone requires for ringtones; mp3 works on Android and most other phones. Both encode at 192 kbps. Default m4r." }
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
    fn output_filename_uses_ringtone_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.mp3", "-ringtone", "m4r"),
            "song-ringtone.m4r"
        );
        assert_eq!(
            filename_with_suffix("track.wav", "-ringtone", "mp3"),
            "track-ringtone.mp3"
        );
    }
}
