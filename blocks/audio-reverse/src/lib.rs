//! gizza-ai/audio-reverse — fetch an audio URL or attachment ref and play it
//! backwards. Part of the audio-input family (`Input::Audio`). `mode` also
//! offers the two combined shapes (original + reversal, or reversal +
//! original), which is how reverse-cymbal risers are made.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Mode/format validation and
//! the pure argv builder live in `core`, shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_reverse_core::{parse_format, parse_mode, plan};
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
    mode: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("mode", ["reverse", "forward-reverse", "reverse-forward"])
                .default("reverse")
                .describe(
                    "What to write out. 'reverse' (default) = the clip backwards. 'forward-reverse' = the original then its reversal. 'reverse-forward' = the reversal then the original (the reverse-cymbal swell into the downbeat). Both combined modes are about twice as long as the input.",
                ),
        )
        .param(
            Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"])
                .default("mp3")
                .describe("Output audio format. Default mp3 (192 kbps); wav and flac are lossless."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioReverse;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-reverse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Play an audio clip backwards, optionally joined with the original",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Reverse an audio clip so it plays backwards (ffmpeg areverse) — sample-exact, no pitch change. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). mode selects what to write: 'reverse' (default) is the clip backwards; 'forward-reverse' plays the original then its reversal; 'reverse-forward' plays the reversal then the original, which is how reverse-cymbal risers build into a downbeat (both combined modes are roughly double the input length). Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a. Input up to 10 MiB; embedded album art is dropped. To reverse only part of a clip, trim it first with trim-audio.",
        parameters = schema_json()
    ),
)]
impl AudioReverse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; mode/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-reverse")?;
    let mode = args.mode.as_deref().unwrap_or("reverse");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates mode + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, mode, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let m = parse_mode(mode).map_err(SkillError::InvalidArgs)?;
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, m.suffix(), fmt.ext());
    let for_llm = format!(
        "{in_filename} written back as {} ({output_size} bytes {})",
        m.describe(),
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
                    "url":    { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":   { "type": "string", "enum": ["reverse", "forward-reverse", "reverse-forward"], "default": "reverse", "description": "What to write out. 'reverse' (default) = the clip backwards. 'forward-reverse' = the original then its reversal. 'reverse-forward' = the reversal then the original (the reverse-cymbal swell into the downbeat). Both combined modes are about twice as long as the input." },
                    "format": { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps); wav and flac are lossless." }
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
    fn output_filename_uses_mode_suffix_and_format_ext() {
        use gizza_ai_audio_reverse_core::Mode;
        assert_eq!(
            filename_with_suffix("guitar.wav", Mode::Reverse.suffix(), "mp3"),
            "guitar-reversed.mp3"
        );
        assert_eq!(
            filename_with_suffix("cymbal.wav", Mode::ReverseForward.suffix(), "wav"),
            "cymbal-reverse-forward.wav"
        );
    }
}
