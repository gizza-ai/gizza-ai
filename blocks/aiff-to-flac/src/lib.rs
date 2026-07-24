//! gizza-ai/aiff-to-flac — fetch an AIFF (or any audio ffmpeg can decode) URL or
//! attachment ref and re-encode it to **lossless FLAC**. Part of the audio-input
//! family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure argv builder +
//! level clamping live in `core`, shared verbatim with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (its native
// registration call requires ::new()). The supporting imports, constants, and
// Args type are only used inside that wasm32-gated impl, so they look "unused"
// under native `cargo test`; `descriptor()`/`schema_json()` stay
// native-compilable so the drift-guard + unit tests can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_aiff_to_flac_core::{plan, DEFAULT_COMPRESSION_LEVEL};
use serde::Deserialize;
use wafer_sdk::*;

// AIFF is uncompressed PCM, so inputs are large — ~10 MiB per minute of 16-bit
// 44.1 kHz stereo. 25 MiB covers ~2.5 minutes; the lossless FLAC output is
// always smaller than its AIFF source, so the same cap is safe for the result.
const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    compression_level: Option<u32>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio).param(
        Param::integer("compression_level")
            .min(0.0)
            .max(12.0)
            .describe(
                "FLAC compression level 0-12 (default 5). Higher = smaller file and slower \
                 encoding; the decoded audio samples are bit-for-bit identical at every level, \
                 so this only trades CPU time for size.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AiffToFlac;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aiff-to-flac",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress AIFF audio to lossless FLAC, preserving tags",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Compress an AIFF audio file into lossless FLAC — a smaller file with bit-for-bit identical samples. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Textual metadata tags (title, artist, album, year, …) are preserved into the FLAC; embedded cover art is dropped. compression_level (0-12, default 5) trades encode speed for file size and never changes the audio. Any audio ffmpeg can decode is accepted, but AIFF → FLAC is the intended use.",
        parameters = schema_json()
    ),
)]
impl AiffToFlac {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; level clamping lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("aiff-to-flac")?;
    let level = args.compression_level.unwrap_or(DEFAULT_COMPRESSION_LEVEL);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). The input extension only names the
    //    scratch file — ffmpeg probes the bytes to detect AIFF/WAV/etc regardless.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("aiff");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, level).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope: FLAC mime, filename keeps the original stem with .flac
    //    (song.aiff → song.flac).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "", "flac");
    let for_llm = format!("compressed {in_filename} to lossless FLAC ({output_size} bytes)");
    build_media_envelope(&output, "audio/flac", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url": { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "compression_level": { "type": "integer", "minimum": 0, "maximum": 12, "description": "FLAC compression level 0-12 (default 5). Higher = smaller file and slower encoding; the decoded audio samples are bit-for-bit identical at every level, so this only trades CPU time for size." }
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
    fn output_filename_keeps_stem_and_swaps_extension() {
        assert_eq!(filename_with_suffix("song.aiff", "", "flac"), "song.flac");
        assert_eq!(filename_with_suffix("field recording.aif", "", "flac"), "field recording.flac");
    }
}
