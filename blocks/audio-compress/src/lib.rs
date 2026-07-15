//! gizza-ai/audio-compress — fetch an audio URL or attachment ref and shrink
//! it by re-encoding at a lower lossy bitrate (mp3/ogg/m4a). Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Format/bitrate validation
//! and the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_compress_core::{parse_format, plan_compress, DEFAULT_BITRATE};
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
    bitrate: Option<u32>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::integer("bitrate")
                .min(32.0)
                .max(320.0)
                .default(96)
                .describe("Target bitrate in kbps (32-320, default 96). Lower means smaller: 64 suits speech, 96-128 keeps music listenable."),
        )
        .param(
            Param::enumv("format", ["mp3", "ogg", "m4a"])
                .default("mp3")
                .describe("Output format, all lossy. Default mp3 (most portable). Lossless wav/flac never shrink a file — use audio-convert for those."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioCompress;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shrink an audio file by re-encoding at a lower bitrate",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Reduce an audio file's size by re-encoding it at a lower lossy bitrate. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). bitrate is the target in kbps (32-320, default 96; 64 suits speech, 96-128 music) — out-of-range values are rejected, not clamped. format is mp3 (default), ogg or m4a; for lossless wav/flac use audio-convert instead. Embedded album art is dropped. If the source's bitrate is already at or below the target, the output won't get meaningfully smaller.",
        parameters = schema_json()
    ),
)]
impl AudioCompress {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-compress")?;
    let kbps = args.bitrate.unwrap_or(DEFAULT_BITRATE);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;
    let input_size = input_bytes.len();

    // 3. Build ffmpeg argv (shared pure core — parses format, validates bitrate).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_compress(&ffmpeg_in, format, kbps).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; tell the LLM how much was
    //    saved (or that the source was already at/below the target bitrate).
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-compressed", fmt.ext());
    let saved = if output_size < input_size && input_size > 0 {
        format!(", {}% smaller", (input_size - output_size) * 100 / input_size)
    } else {
        " — source bitrate was already at or below the target".to_string()
    };
    let for_llm = format!(
        "compressed {in_filename} from {input_size} to {output_size} bytes at {kbps} kbps {}{saved}",
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
                    "bitrate": { "type": "integer", "minimum": 32, "maximum": 320, "default": 96, "description": "Target bitrate in kbps (32-320, default 96). Lower means smaller: 64 suits speech, 96-128 keeps music listenable." },
                    "format":  { "type": "string", "enum": ["mp3", "ogg", "m4a"], "default": "mp3", "description": "Output format, all lossy. Default mp3 (most portable). Lossless wav/flac never shrink a file — use audio-convert for those." }
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
    fn output_filename_gets_compressed_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.wav", "-compressed", "mp3"),
            "song-compressed.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-compressed", "ogg"),
            "voice memo-compressed.ogg"
        );
    }
}
