//! gizza-ai/aiff-to-wav — fetch an AIFF/AIFC (or any audio ffmpeg can decode)
//! URL or attachment ref and re-container it as a RIFF/WAVE `.wav`. Part of the
//! audio-input family (`Input::Audio`).
//!
//! At a matching bit depth the conversion is lossless: AIFF and WAV both carry
//! linear PCM and differ only in byte order (big- vs little-endian), so the
//! decoded sample values come out identical. The argv construction + validation
//! live in `core`, shared verbatim with the page's `build_argv`.
//!
//! The chat schema is derived from `descriptor()` (single source — the same
//! shape backs chat, CLI, and the page form); the handler delegates
//! source-resolution, ffmpeg dispatch, and envelope-building to `block_utils`.

// The #[wafer_block] macro emits the impl gated to wasm32 (its native
// registration call requires ::new()). The supporting imports, constants, and
// Args type are only used inside that wasm32-gated impl, so they look "unused"
// under native `cargo test`; `descriptor()`/`schema_json()` stay
// native-compilable so the drift-guard + unit tests can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_aiff_to_wav_core::{
    plan, BIT_DEPTHS, CHANNELS, DEFAULT_BIT_DEPTH, DEFAULT_CHANNELS, DEFAULT_KEEP_METADATA,
    DEFAULT_SAMPLE_RATE, SAMPLE_RATES,
};
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

// AIFF is uncompressed PCM — ~10 MiB per minute of 16-bit 44.1 kHz stereo, so
// 25 MiB covers ~2.5 minutes. The WAV output can be LARGER than its source when
// the default 24-bit depth widens a 16-bit master (1.5x), so the output cap is
// raised accordingly rather than mirroring the input cap.
const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 50 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    bit_depth: Option<String>,
    #[serde(default)]
    sample_rate: Option<String>,
    #[serde(default)]
    channels: Option<String>,
    #[serde(default)]
    keep_metadata: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("bit_depth", BIT_DEPTHS)
                .default(DEFAULT_BIT_DEPTH)
                .describe(
                    "Output PCM encoding (default 24). 16/24/32 are linear integer PCM, float32 \
                     is IEEE 32-bit float, alaw/mulaw are G.711 telephony encodings. 24 never \
                     truncates a 16- or 24-bit AIFF master; picking a depth below the source's \
                     loses bits.",
                ),
        )
        .param(
            Param::enumv("sample_rate", SAMPLE_RATES)
                .default(DEFAULT_SAMPLE_RATE)
                .describe(
                    "Output sample rate in Hz, or \"keep\" (default) to pass the source rate \
                     through untouched. Resampling is never lossless — only set this when a \
                     target system demands a specific rate.",
                ),
        )
        .param(
            Param::enumv("channels", CHANNELS)
                .default(DEFAULT_CHANNELS)
                .describe(
                    "Output channel layout: \"keep\" (default) passes the source layout through, \
                     \"mono\" downmixes to 1 channel, \"stereo\" forces 2. Downmixing is not \
                     reversible.",
                ),
        )
        .param(
            Param::boolean("keep_metadata")
                .default(DEFAULT_KEEP_METADATA)
                .describe(
                    "Copy the source's textual tags (title, artist, album, year, …) into the \
                     WAV's LIST/INFO chunk. Set false to strip them for a clean delivery file. \
                     Embedded cover art is always dropped — WAV has no standard picture chunk.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AiffToWav;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aiff-to-wav",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert AIFF audio to WAV with identical PCM samples",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert an AIFF/AIFC (Mac, Logic, Pro Tools) audio file to a RIFF/WAVE .wav. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Both formats carry uncompressed linear PCM and differ only in byte order, so at a matching bit depth the samples are bit-for-bit identical. bit_depth (default 24) picks the output PCM encoding and is always sent explicitly, because ffmpeg's wav muxer would otherwise default to 16-bit and silently truncate a 24-bit master. sample_rate and channels default to \"keep\" so nothing is resampled or downmixed. keep_metadata (default true) copies textual tags into the WAV LIST/INFO chunk; cover art is always dropped. Any audio ffmpeg can decode is accepted, but AIFF → WAV is the intended use.",
        parameters = schema_json()
    ),
)]
impl AiffToWav {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; every value is validated inside core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("aiff-to-wav")?;
    let bit_depth = args.bit_depth.unwrap_or_else(|| DEFAULT_BIT_DEPTH.to_string());
    let sample_rate = args
        .sample_rate
        .unwrap_or_else(|| DEFAULT_SAMPLE_RATE.to_string());
    let channels = args.channels.unwrap_or_else(|| DEFAULT_CHANNELS.to_string());
    let keep_metadata = args.keep_metadata.unwrap_or(DEFAULT_KEEP_METADATA);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). The input extension only names the
    //    scratch file — ffmpeg probes the bytes to detect AIFF/AIFC/WAV regardless.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("aiff");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        &bit_depth,
        &sample_rate,
        &channels,
        keep_metadata,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope: WAV mime, filename keeps the original stem with .wav
    //    (session.aiff → session.wav).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "", "wav");
    let for_llm = format!("converted {in_filename} to WAV ({output_size} bytes)");
    build_media_envelope(&output, "audio/wav", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "bit_depth": {
                        "type": "string",
                        "enum": ["16", "24", "32", "float32", "alaw", "mulaw"],
                        "default": "24",
                        "description": "Output PCM encoding (default 24). 16/24/32 are linear integer PCM, float32 is IEEE 32-bit float, alaw/mulaw are G.711 telephony encodings. 24 never truncates a 16- or 24-bit AIFF master; picking a depth below the source's loses bits."
                    },
                    "sample_rate": {
                        "type": "string",
                        "enum": ["keep", "8000", "16000", "22050", "44100", "48000", "88200", "96000", "192000"],
                        "default": "keep",
                        "description": "Output sample rate in Hz, or \"keep\" (default) to pass the source rate through untouched. Resampling is never lossless — only set this when a target system demands a specific rate."
                    },
                    "channels": {
                        "type": "string",
                        "enum": ["keep", "mono", "stereo"],
                        "default": "keep",
                        "description": "Output channel layout: \"keep\" (default) passes the source layout through, \"mono\" downmixes to 1 channel, \"stereo\" forces 2. Downmixing is not reversible."
                    },
                    "keep_metadata": {
                        "type": "boolean",
                        "default": true,
                        "description": "Copy the source's textual tags (title, artist, album, year, …) into the WAV's LIST/INFO chunk. Set false to strip them for a clean delivery file. Embedded cover art is always dropped — WAV has no standard picture chunk."
                    }
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

    /// The advertised enum values in the schema must be exactly the ones core
    /// accepts — a page `<select>` option core rejects would be a dead control.
    #[test]
    fn every_advertised_enum_value_is_accepted_by_core() {
        for depth in BIT_DEPTHS {
            assert!(plan("in.aiff", depth, "keep", "keep", true).is_ok(), "{depth}");
        }
        for rate in SAMPLE_RATES {
            assert!(plan("in.aiff", "24", rate, "keep", true).is_ok(), "{rate}");
        }
        for layout in CHANNELS {
            assert!(plan("in.aiff", "24", "keep", layout, true).is_ok(), "{layout}");
        }
    }

    #[test]
    fn output_filename_keeps_stem_and_swaps_extension() {
        assert_eq!(filename_with_suffix("session.aiff", "", "wav"), "session.wav");
        assert_eq!(
            filename_with_suffix("vocal take 3.aif", "", "wav"),
            "vocal take 3.wav"
        );
    }
}
