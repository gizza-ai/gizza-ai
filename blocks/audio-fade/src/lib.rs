//! gizza-ai/audio-fade — fetch an audio URL or attachment ref and add a
//! fade-in and/or fade-out. Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Fade validation and the
//! pure argv builder (incl. the duration-free areverse fade-out trick) live in
//! `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_fade_core::{fmt_num, parse_format, plan_fade, DEFAULT_FADE_S};
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
    fade_in: Option<f64>,
    #[serde(default)]
    fade_out: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("fade_in")
                .min(0.0)
                .max(30.0)
                .default(3.0)
                .describe("Fade-in length in seconds at the start (0-30, default 3). 0 skips the fade-in."),
        )
        .param(
            Param::number("fade_out")
                .min(0.0)
                .max(30.0)
                .default(3.0)
                .describe("Fade-out length in seconds at the end (0-30, default 3). 0 skips the fade-out."),
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
struct AudioFade;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-fade",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a fade-in and fade-out to an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Add a smooth fade-in at the start and/or fade-out at the end of an audio file. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). fade_in and fade_out are lengths in seconds (0-30, both default 3); set one to 0 to fade only the other side — both at 0 is rejected. Fades longer than the track just fade across the whole file. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioFade {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; fade/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-fade")?;
    let fade_in = args.fade_in.unwrap_or(DEFAULT_FADE_S);
    let fade_out = args.fade_out.unwrap_or(DEFAULT_FADE_S);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates fades + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_fade(&ffmpeg_in, fade_in, fade_out, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied fades.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fade", fmt.ext());
    let what = match (fade_in > 0.0, fade_out > 0.0) {
        (true, true) => format!(
            "{} s fade-in + {} s fade-out",
            fmt_num(fade_in),
            fmt_num(fade_out)
        ),
        (true, false) => format!("{} s fade-in", fmt_num(fade_in)),
        _ => format!("{} s fade-out", fmt_num(fade_out)),
    };
    let for_llm = format!(
        "added {what} to {in_filename} ({output_size} bytes {})",
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
    /// wording), so the expected JSON uses that shared wording. Number-param
    /// defaults serialize as floats (`3.0`), whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fade_in":  { "type": "number", "minimum": 0, "maximum": 30, "default": 3.0, "description": "Fade-in length in seconds at the start (0-30, default 3). 0 skips the fade-in." },
                    "fade_out": { "type": "number", "minimum": 0, "maximum": 30, "default": 3.0, "description": "Fade-out length in seconds at the end (0-30, default 3). 0 skips the fade-out." },
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
    fn output_filename_uses_fade_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.wav", "-fade", "mp3"),
            "song-fade.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-fade", "ogg"),
            "voice memo-fade.ogg"
        );
    }
}
