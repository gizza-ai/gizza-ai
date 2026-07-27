//! gizza-ai/audio-compressor — fetch an audio URL or attachment ref and apply
//! dynamic-range compression with ffmpeg's `acompressor` filter (threshold,
//! ratio, attack, release + make-up gain). Part of the audio-input family
//! (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Range validation and the
//! pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_compressor_core::{parse_format, plan_compress};
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
    threshold: Option<f64>,
    #[serde(default)]
    ratio: Option<f64>,
    #[serde(default)]
    attack: Option<f64>,
    #[serde(default)]
    release: Option<f64>,
    #[serde(default)]
    makeup: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("threshold")
                .min(-60.0)
                .max(0.0)
                .default(-20.0)
                .describe("Threshold in dB below full scale where compression starts. Lower (e.g. -30) compresses more of the signal; higher (e.g. -10) only catches the loudest peaks. Default -20."),
        )
        .param(
            Param::number("ratio")
                .min(1.0)
                .max(20.0)
                .default(4.0)
                .describe("Compression ratio (ratio:1): how hard signal above the threshold is squeezed. 2 is gentle, 4 is a firm even level, 10+ approaches limiting. 1 = no compression. Default 4."),
        )
        .param(
            Param::number("attack")
                .min(0.01)
                .max(2000.0)
                .default(20.0)
                .describe("Attack time in milliseconds — how fast the compressor clamps down once the signal passes the threshold. Fast (5) tames transients, slow (30+) lets punch through. Default 20."),
        )
        .param(
            Param::number("release")
                .min(0.01)
                .max(9000.0)
                .default(250.0)
                .describe("Release time in milliseconds — how fast it lets go once the signal drops below the threshold. Short (60) for speech, long (250+) for smoother music. Default 250."),
        )
        .param(
            Param::number("makeup")
                .min(0.0)
                .max(24.0)
                .default(0.0)
                .describe("Make-up gain in dB added after compression to restore loudness the compressor pulled down. 0 leaves the level as compressed; try 3-6 to bring it back up. Default 0."),
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
struct AudioCompressor;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-compressor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress audio dynamic range with threshold, ratio, attack and release",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply dynamic-range compression to an audio file so loud and quiet passages sit closer together. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Controls: threshold (dB, -60..0, where compression starts), ratio (1..20, how hard it squeezes above the threshold), attack (ms, 0.01..2000, how fast it clamps down), release (ms, 0.01..9000, how fast it lets go) and makeup (dB, 0..24, gain added afterward to restore loudness). A ratio of 1 with 0 dB make-up gain is a no-op and is rejected. This is loudness/dynamics compression, not file-size compression. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioCompressor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; control/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-compressor")?;
    let threshold = args.threshold.unwrap_or(-20.0);
    let ratio = args.ratio.unwrap_or(4.0);
    let attack = args.attack.unwrap_or(20.0);
    let release = args.release.unwrap_or(250.0);
    let makeup = args.makeup.unwrap_or(0.0);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates controls + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_compress(
        &ffmpeg_in, threshold, ratio, attack, release, makeup, format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied settings
    //    so the LLM can echo what changed. `{:+}` prints 6.0 as "+6".
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-compressed", fmt.ext());
    let makeup_note = if makeup != 0.0 {
        format!(", makeup {makeup:+} dB")
    } else {
        String::new()
    };
    let for_llm = format!(
        "compressed {in_filename}: threshold {threshold} dB, ratio {ratio}:1, attack {attack} ms, release {release} ms{makeup_note} ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats (`-20.0`),
    /// whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "number", "minimum": -60, "maximum": 0, "default": -20.0, "description": "Threshold in dB below full scale where compression starts. Lower (e.g. -30) compresses more of the signal; higher (e.g. -10) only catches the loudest peaks. Default -20." },
                    "ratio":     { "type": "number", "minimum": 1, "maximum": 20, "default": 4.0, "description": "Compression ratio (ratio:1): how hard signal above the threshold is squeezed. 2 is gentle, 4 is a firm even level, 10+ approaches limiting. 1 = no compression. Default 4." },
                    "attack":    { "type": "number", "minimum": 0.01, "maximum": 2000, "default": 20.0, "description": "Attack time in milliseconds — how fast the compressor clamps down once the signal passes the threshold. Fast (5) tames transients, slow (30+) lets punch through. Default 20." },
                    "release":   { "type": "number", "minimum": 0.01, "maximum": 9000, "default": 250.0, "description": "Release time in milliseconds — how fast it lets go once the signal drops below the threshold. Short (60) for speech, long (250+) for smoother music. Default 250." },
                    "makeup":    { "type": "number", "minimum": 0, "maximum": 24, "default": 0.0, "description": "Make-up gain in dB added after compression to restore loudness the compressor pulled down. 0 leaves the level as compressed; try 3-6 to bring it back up. Default 0." },
                    "format":    { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_compressed_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("song.wav", "-compressed", "mp3"),
            "song-compressed.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-compressed", "flac"),
            "voice memo-compressed.flac"
        );
    }
}
