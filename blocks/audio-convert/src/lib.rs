//! gizza-ai/audio-convert — fetch an audio URL or attachment ref and re-encode
//! it to mp3, wav, ogg, flac or m4a (lossy targets take a bitrate). Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Format/bitrate parsing and
//! the pure argv builder live in `core`, shared with the page.

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
use gizza_ai_audio_convert_core::{parse_format, plan_convert, DEFAULT_BITRATE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    format: String,
    #[serde(default)]
    bitrate: Option<u32>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"])
                .required()
                .describe("Target audio format: mp3/ogg/m4a are lossy (take a bitrate), wav/flac are lossless."),
        )
        .param(
            Param::integer("bitrate")
                .min(32.0)
                .max(320.0)
                .describe("Bitrate in kbps for lossy targets (32-320, default 192). Ignored for lossless wav/flac."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioConvert;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert audio between mp3, wav, ogg, flac and m4a",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert an audio file to mp3, wav, ogg, flac or m4a. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Lossy targets (mp3/ogg/m4a) take a bitrate in kbps (32-320, default 192); wav/flac are lossless and ignore it. Any input ffmpeg can decode works; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; format/bitrate validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-convert")?;
    let kbps = args.bitrate.unwrap_or(DEFAULT_BITRATE);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — parses format, clamps bitrate).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_convert(&ffmpeg_in, &args.format, kbps).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; filename keeps the original
    //    stem with the new extension (song.mp3 → song.wav).
    let fmt = parse_format(&args.format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "", fmt.ext());
    let for_llm = format!(
        "converted {in_filename} to {} ({output_size} bytes)",
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
                    "url":     { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format":  { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "description": "Target audio format: mp3/ogg/m4a are lossy (take a bitrate), wav/flac are lossless." },
                    "bitrate": { "type": "integer", "minimum": 32, "maximum": 320, "description": "Bitrate in kbps for lossy targets (32-320, default 192). Ignored for lossless wav/flac." }
                },
                "additionalProperties": false,
                "required": ["format"],
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
    fn output_filename_keeps_stem_and_swaps_extension() {
        assert_eq!(filename_with_suffix("song.mp3", "", "wav"), "song.wav");
        assert_eq!(filename_with_suffix("voice memo.m4a", "", "mp3"), "voice memo.mp3");
    }
}
