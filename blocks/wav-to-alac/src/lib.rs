//! gizza-ai/wav-to-alac — fetch a WAV (or any audio ffmpeg can decode) URL or
//! attachment ref and encode it to **ALAC (Apple Lossless) in an `.m4a`
//! container** — the lossless form iOS / Apple Music imports. Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure argv builder and
//! selector parsing live in `core`, shared verbatim with the page.

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
use gizza_ai_wav_to_alac_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

// WAV is uncompressed PCM — ~10 MiB per minute of 16-bit 44.1 kHz stereo, and
// ~30 MiB per minute at 24-bit/96 kHz. 25 MiB covers a couple of minutes of CD
// audio. ALAC typically lands at 40-60% of the source size, but upsampling
// (`sample_rate`) or widening (`bit_depth 24`) can make the result LARGER than
// the input, so the output cap is deliberately roomier than the input cap.
const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 60 * 1024 * 1024;

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
///
/// Every selector's first choice is `source`, which omits its ffmpeg flag
/// entirely — the default run is a straight lossless re-wrap of the source PCM.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("bit_depth", ["source", "16", "24"])
                .default("source")
                .describe(
                    "Output bit depth: source (default, keep the input's depth), 16 (CD depth) \
                     or 24 (hi-res). ALAC only supports these two depths; the conversion stays \
                     lossless either way, but 16 discards the low bits of a 24-bit master.",
                ),
        )
        .param(
            Param::enumv(
                "sample_rate",
                [
                    "source", "44100", "48000", "88200", "96000", "176400", "192000",
                ],
            )
            .default("source")
            .describe(
                "Output sample rate in Hz: source (default, keep the input's rate) or one of \
                 44100, 48000, 88200, 96000, 176400, 192000. Resampling is NOT lossless — keep \
                 source unless a target device needs a specific rate.",
            ),
        )
        .param(
            Param::enumv("channels", ["source", "mono", "stereo"])
                .default("source")
                .describe(
                    "Channel layout: source (default, keep the input's channels), mono (fold to \
                     1 channel) or stereo (force 2 channels). Use stereo to make a surround \
                     master play reliably on Apple devices.",
                ),
        )
        .param(
            Param::boolean("keep_metadata")
                .default(true)
                .describe(
                    "Copy the source's textual tags (title, artist, album, year, …) into the \
                     .m4a. Default true; set false for a clean, tag-free file. Embedded cover \
                     art is always dropped.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WavToAlac;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wav-to-alac",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode WAV audio to Apple Lossless (ALAC) in an .m4a container",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Encode a WAV file to Apple Lossless (ALAC) inside an .m4a container — the lossless format iOS, iTunes and Apple Music import. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). ALAC has no quality setting: the decoded samples are bit-for-bit identical to the source. bit_depth (source|16|24), sample_rate (source|44100|48000|88200|96000|176400|192000) and channels (source|mono|stereo) all default to source, which leaves the audio untouched; keep_metadata (default true) copies textual tags into the .m4a. The result is always faststart-muxed so it streams immediately. Any audio ffmpeg can decode is accepted, but WAV → ALAC is the intended use.",
        parameters = schema_json()
    ),
)]
impl WavToAlac {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; selector validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("wav-to-alac")?;
    let bit_depth = args.bit_depth.unwrap_or_else(|| "source".to_string());
    let sample_rate = args.sample_rate.unwrap_or_else(|| "source".to_string());
    let channels = args.channels.unwrap_or_else(|| "source".to_string());
    let keep_metadata = args.keep_metadata.unwrap_or(true);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). The input extension only names
    //    the scratch file — ffmpeg probes the bytes to detect WAV/AIFF/etc.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("wav");
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

    // 5. Envelope: ALAC rides in an MP4 container, so the mime is audio/mp4 and
    //    the filename keeps the original stem with .m4a (master.wav →
    //    master.m4a).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "", "m4a");
    let for_llm = format!("encoded {in_filename} to Apple Lossless ALAC ({output_size} bytes)");
    build_media_envelope(&output, "audio/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
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
                        "enum": ["source", "16", "24"],
                        "default": "source",
                        "description": "Output bit depth: source (default, keep the input's depth), 16 (CD depth) or 24 (hi-res). ALAC only supports these two depths; the conversion stays lossless either way, but 16 discards the low bits of a 24-bit master."
                    },
                    "sample_rate": {
                        "type": "string",
                        "enum": ["source", "44100", "48000", "88200", "96000", "176400", "192000"],
                        "default": "source",
                        "description": "Output sample rate in Hz: source (default, keep the input's rate) or one of 44100, 48000, 88200, 96000, 176400, 192000. Resampling is NOT lossless — keep source unless a target device needs a specific rate."
                    },
                    "channels": {
                        "type": "string",
                        "enum": ["source", "mono", "stereo"],
                        "default": "source",
                        "description": "Channel layout: source (default, keep the input's channels), mono (fold to 1 channel) or stereo (force 2 channels). Use stereo to make a surround master play reliably on Apple devices."
                    },
                    "keep_metadata": {
                        "type": "boolean",
                        "default": true,
                        "description": "Copy the source's textual tags (title, artist, album, year, …) into the .m4a. Default true; set false for a clean, tag-free file. Embedded cover art is always dropped."
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

    /// Every advertised enum choice must be one the core actually accepts —
    /// otherwise the page's `<select>` could offer a value that errors at run
    /// time.
    #[test]
    fn every_advertised_enum_choice_is_accepted_by_core() {
        let d = descriptor();
        let choices = |name: &str| -> Vec<String> {
            let json: serde_json::Value = serde_json::from_str(&d.to_schema_json()).unwrap();
            json["properties"][name]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for v in choices("bit_depth") {
            plan("in.wav", &v, "source", "source", true)
                .unwrap_or_else(|e| panic!("bit_depth {v}: {e}"));
        }
        for v in choices("sample_rate") {
            plan("in.wav", "source", &v, "source", true)
                .unwrap_or_else(|e| panic!("sample_rate {v}: {e}"));
        }
        for v in choices("channels") {
            plan("in.wav", "source", "source", &v, true)
                .unwrap_or_else(|e| panic!("channels {v}: {e}"));
        }
    }

    #[test]
    fn output_filename_keeps_stem_and_swaps_extension() {
        assert_eq!(filename_with_suffix("master.wav", "", "m4a"), "master.m4a");
        assert_eq!(
            filename_with_suffix("live take 2.WAV", "", "m4a"),
            "live take 2.m4a"
        );
    }
}
