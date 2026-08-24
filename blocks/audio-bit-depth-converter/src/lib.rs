//! gizza-ai/audio-bit-depth-converter — fetch an audio URL or attachment ref and
//! requantize it to a chosen PCM bit depth (8 / 16 / 24 / 32-bit float) with a
//! selectable dither algorithm on down-conversion, writing lossless wav or flac.
//! Part of the audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Depth/dither/format parsing
//! and the pure argv builder live in `core`, shared verbatim with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call requiring ::new()). The supporting imports, constants, and
// Args type are only used inside that wasm32-gated impl, so they look "unused"
// during native unit tests. `descriptor()`/`schema_json()` stay native-
// compilable so the drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_bit_depth_converter_core::{
    parse_depth, parse_format, plan_convert, DEFAULT_DEPTH, DEFAULT_DITHER, DEFAULT_FORMAT,
};
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

/// `keep_metadata` defaults to true (ffmpeg's own behavior) when omitted.
fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    bit_depth: Option<String>,
    #[serde(default)]
    dither: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default = "default_true")]
    keep_metadata: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("bit_depth", ["8", "16", "24", "32f"])
                .default("16")
                .describe(
                    "Target PCM bit depth (default 16). 8 = 8-bit unsigned (48 dB range, tiny/retro); \
                     16 = CD and streaming delivery standard (96 dB); 24 = studio/mastering standard \
                     (144 dB); 32f = 32-bit float for DAW interchange. flac stores 16 and 24 only.",
                ),
        )
        .param(
            Param::enumv(
                "dither",
                [
                    "none",
                    "rectangular",
                    "triangular",
                    "triangular_hp",
                    "lipshitz",
                    "f_weighted",
                    "modified_e_weighted",
                    "improved_e_weighted",
                    "shibata",
                    "low_shibata",
                    "high_shibata",
                ],
            )
            .default("triangular")
            .describe(
                "Dither added while requantizing DOWN, so truncation error becomes benign noise \
                 instead of audible distortion (default triangular = standard TPDF). none = plain \
                 truncation; rectangular = flat RPDF; triangular_hp = TPDF with a high-pass; \
                 lipshitz/f_weighted/modified_e_weighted/improved_e_weighted/shibata/low_shibata/\
                 high_shibata are noise-shaped variants that hide the noise where the ear is least \
                 sensitive (shibata is the usual pick for 16-bit masters). Ignored when the target \
                 is 32f, where nothing is truncated.",
            ),
        )
        .param(
            Param::enumv("format", ["wav", "flac"])
                .default("wav")
                .describe(
                    "Output container (default wav). Both are lossless: wav carries every depth and \
                     previews everywhere; flac is compressed and supports 16-bit and 24-bit only.",
                ),
        )
        .param(
            Param::boolean("keep_metadata")
                .default(true)
                .describe(
                    "Copy title/artist/album tags from the source into the output (default true). \
                     Set false to write a clean file with no metadata.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioBitDepthConverter;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-bit-depth-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change audio PCM bit depth (24-bit to 16-bit and more) with dithering",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Change an audio file's PCM bit depth — the classic case being a 24-bit master down to dithered 16-bit for CD or streaming delivery. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). bit_depth is 8, 16 (default), 24 or 32f (32-bit float). dither picks the noise added while requantizing down (default triangular = standard TPDF; none = plain truncation; shibata and friends are noise-shaped); it is ignored when the target is 32f because nothing is truncated. format is wav (default, carries every depth) or flac (16-bit and 24-bit only). keep_metadata copies source tags (default true). Sample RATE is untouched — use audio-resampler for that. Any input ffmpeg can decode works; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioBitDepthConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; depth/dither/format validation lives in core's plan.
    let args: Args =
        serde_json::from_slice(&body).invalid_args("audio-bit-depth-converter")?;
    let bit_depth = args.bit_depth.as_deref().unwrap_or(DEFAULT_DEPTH);
    let dither = args.dither.as_deref().unwrap_or(DEFAULT_DITHER);
    let format = args.format.as_deref().unwrap_or(DEFAULT_FORMAT);

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — parses every param).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("wav");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_convert(
        &ffmpeg_in,
        bit_depth,
        dither,
        format,
        args.keep_metadata,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; filename keeps the original
    //    stem with a depth suffix + the new extension (song.flac → song-16bit.wav).
    let depth = parse_depth(bit_depth).map_err(SkillError::InvalidArgs)?;
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, depth.suffix(), fmt.ext());
    let for_llm = format!(
        "converted {in_filename} to {} {} ({output_size} bytes)",
        depth.label(),
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
                    "url":           { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":           { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "bit_depth":     { "type": "string", "enum": ["8", "16", "24", "32f"], "default": "16", "description": "Target PCM bit depth (default 16). 8 = 8-bit unsigned (48 dB range, tiny/retro); 16 = CD and streaming delivery standard (96 dB); 24 = studio/mastering standard (144 dB); 32f = 32-bit float for DAW interchange. flac stores 16 and 24 only." },
                    "dither":        { "type": "string", "enum": ["none", "rectangular", "triangular", "triangular_hp", "lipshitz", "f_weighted", "modified_e_weighted", "improved_e_weighted", "shibata", "low_shibata", "high_shibata"], "default": "triangular", "description": "Dither added while requantizing DOWN, so truncation error becomes benign noise instead of audible distortion (default triangular = standard TPDF). none = plain truncation; rectangular = flat RPDF; triangular_hp = TPDF with a high-pass; lipshitz/f_weighted/modified_e_weighted/improved_e_weighted/shibata/low_shibata/high_shibata are noise-shaped variants that hide the noise where the ear is least sensitive (shibata is the usual pick for 16-bit masters). Ignored when the target is 32f, where nothing is truncated." },
                    "format":        { "type": "string", "enum": ["wav", "flac"], "default": "wav", "description": "Output container (default wav). Both are lossless: wav carries every depth and previews everywhere; flac is compressed and supports 16-bit and 24-bit only." },
                    "keep_metadata": { "type": "boolean", "default": true, "description": "Copy title/artist/album tags from the source into the output (default true). Set false to write a clean file with no metadata." }
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
    fn output_filename_adds_depth_suffix_and_swaps_extension() {
        assert_eq!(
            filename_with_suffix("master.flac", "-16bit", "wav"),
            "master-16bit.wav"
        );
        assert_eq!(
            filename_with_suffix("live take 2.wav", "-24bit", "flac"),
            "live take 2-24bit.flac"
        );
    }

    #[test]
    fn omitted_optional_args_fall_back_to_the_documented_defaults() {
        let args: Args =
            serde_json::from_str(r#"{"url":"https://example.com/a.wav"}"#).unwrap();
        assert_eq!(args.bit_depth.as_deref().unwrap_or(DEFAULT_DEPTH), "16");
        assert_eq!(args.dither.as_deref().unwrap_or(DEFAULT_DITHER), "triangular");
        assert_eq!(args.format.as_deref().unwrap_or(DEFAULT_FORMAT), "wav");
        assert!(args.keep_metadata, "metadata is kept unless asked otherwise");
    }

    #[test]
    fn explicit_args_including_a_false_boolean_round_trip() {
        let args: Args = serde_json::from_str(
            r#"{"url":"https://example.com/a.wav","bit_depth":"24","dither":"shibata","format":"flac","keep_metadata":false}"#,
        )
        .unwrap();
        assert_eq!(args.bit_depth.as_deref(), Some("24"));
        assert_eq!(args.dither.as_deref(), Some("shibata"));
        assert_eq!(args.format.as_deref(), Some("flac"));
        assert!(!args.keep_metadata);
        // The same values must plan a valid argv end to end.
        let (argv, out) = plan_convert("in.wav", "24", "shibata", "flac", false).unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.iter().any(|a| a == "-map_metadata"));
    }
}
