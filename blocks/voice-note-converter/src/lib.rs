//! gizza-ai/voice-note-converter — convert chat voice messages both directions:
//! decode an incoming voice note (Opus/OGG from WhatsApp/Telegram/Signal, or any
//! audio ffmpeg can decode) to mp3/wav, and encode mp3/wav BACK into a real
//! `.opus` voice note (Opus codec in an Ogg container — what messaging apps
//! recognise). Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch and envelope-building to `block_utils`. Format/bitrate parsing and
//! the pure argv builder live in `core`, shared verbatim with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the native
// registration call it generates requires ::new(), which skill-style impls lack).
// The supporting imports, constants and the Args type are only used inside that
// wasm32-gated impl, so they read as "unused" under native unit tests.
// `descriptor()` / `schema_json()` stay native-compilable so the drift-guard test
// can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_voice_note_converter_core::{parse_format, plan_convert};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB — voice notes are tiny
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

fn default_mono() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    format: String,
    #[serde(default)]
    bitrate: Option<u32>,
    #[serde(default = "default_mono")]
    mono: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("format", ["opus", "mp3", "wav"])
                .required()
                .describe(
                    "Target format: opus = a real messaging-app voice note (Opus in Ogg), \
                     mp3 = plays everywhere, wav = uncompressed for editing.",
                ),
        )
        .param(
            Param::integer("bitrate")
                .min(6.0)
                .max(320.0)
                .describe(
                    "Bitrate in kbps for the lossy targets (opus 6-256, mp3 32-320); \
                     clamped per format. Defaults: opus 32, mp3 128. Ignored for wav.",
                ),
        )
        .param(
            Param::boolean("mono")
                .default(true)
                .describe(
                    "Downmix to a single channel (the voice-note standard; also selects \
                     libopus' speech-tuned `voip` mode). Default true; set false to keep stereo.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VoiceNoteConverter;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/voice-note-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert chat voice notes to mp3/wav and back to Opus",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a chat voice message. Decode an incoming voice note (Opus/OGG from WhatsApp, Telegram, Signal — or any audio ffmpeg can decode) to mp3 or wav, or encode mp3/wav back into a real .opus voice note (Opus codec in an Ogg container). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). format is opus|mp3|wav (required). bitrate (kbps) applies to the lossy targets — opus 6-256 (default 32), mp3 32-320 (default 128) — and is clamped per format; wav ignores it. mono (default true) downmixes to one channel and tunes Opus for speech. Embedded cover art is dropped.",
        parameters = schema_json()
    ),
)]
impl VoiceNoteConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; format/bitrate validation lives in core's plan_convert.
    let args: Args = serde_json::from_slice(&body).invalid_args("voice-note-converter")?;

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — parses format, clamps bitrate,
    //    picks the voice-tuned Opus application when mono).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("ogg");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_convert(&ffmpeg_in, &args.format, args.bitrate, args.mono)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; filename keeps the original
    //    stem with the new extension (voice.ogg → voice.mp3 / memo.wav → memo.opus).
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
    /// property descriptions are centralized in `to_schema_json` (Audio wording).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format":  { "type": "string", "enum": ["opus", "mp3", "wav"], "description": "Target format: opus = a real messaging-app voice note (Opus in Ogg), mp3 = plays everywhere, wav = uncompressed for editing." },
                    "bitrate": { "type": "integer", "minimum": 6, "maximum": 320, "description": "Bitrate in kbps for the lossy targets (opus 6-256, mp3 32-320); clamped per format. Defaults: opus 32, mp3 128. Ignored for wav." },
                    "mono":    { "type": "boolean", "default": true, "description": "Downmix to a single channel (the voice-note standard; also selects libopus' speech-tuned `voip` mode). Default true; set false to keep stereo." }
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
        assert_eq!(filename_with_suffix("voice.ogg", "", "mp3"), "voice.mp3");
        assert_eq!(filename_with_suffix("memo.mp3", "", "opus"), "memo.opus");
    }
}
