//! gizza-ai/audio-eq — fetch an audio URL or attachment ref and apply a
//! three-band equalizer (bass/mid/treble gains in dB). Part of the
//! audio-input family (`Input::Audio`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Gain validation and the
//! pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` and the block-local helpers remain native-compilable so the
// drift-guard + unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_eq_core::{parse_format, plan_eq};
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
    bass: Option<f64>,
    #[serde(default)]
    mid: Option<f64>,
    #[serde(default)]
    treble: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("bass")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Bass gain in dB (low shelf, ~100 Hz): 6 warms it up, -6 tames boominess. 0 leaves the band unchanged."),
        )
        .param(
            Param::number("mid")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Mid gain in dB (1 kHz peaking band): boost for vocal presence, cut for a boxy sound. 0 leaves the band unchanged."),
        )
        .param(
            Param::number("treble")
                .min(-20.0)
                .max(20.0)
                .default(0.0)
                .describe("Treble gain in dB (high shelf, ~3 kHz): 4 brightens, -4 softens hiss/harshness. 0 leaves the band unchanged."),
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
struct AudioEq;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-eq",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Equalize audio with bass, mid and treble gains",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a three-band equalizer to an audio file. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). bass (low shelf ~100 Hz), mid (1 kHz peaking band) and treble (high shelf ~3 kHz) are gains in dB from -20 to 20; 0 leaves a band unchanged and at least one band must be non-zero. Typical fixes: bass 6 to warm a thin recording, mid -4 for a boxy voice, treble 4 to brighten a dull one. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl AudioEq {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; gain/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-eq")?;
    let bass = args.bass.unwrap_or(0.0);
    let mid = args.mid.unwrap_or(0.0);
    let treble = args.treble.unwrap_or(0.0);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates gains + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_eq(&ffmpeg_in, bass, mid, treble, format).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied bands
    //    so the LLM can echo what changed. `{:+}` prints 6.0 as "+6".
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-eq", fmt.ext());
    let bands: Vec<String> = [("bass", bass), ("mid", mid), ("treble", treble)]
        .iter()
        .filter(|(_, g)| *g != 0.0)
        .map(|(name, g)| format!("{name} {g:+} dB"))
        .collect();
    let for_llm = format!(
        "equalized {in_filename}: {} ({output_size} bytes {})",
        bands.join(", "),
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
    /// defaults serialize as floats (`0.0`), whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "bass":   { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Bass gain in dB (low shelf, ~100 Hz): 6 warms it up, -6 tames boominess. 0 leaves the band unchanged." },
                    "mid":    { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Mid gain in dB (1 kHz peaking band): boost for vocal presence, cut for a boxy sound. 0 leaves the band unchanged." },
                    "treble": { "type": "number", "minimum": -20, "maximum": 20, "default": 0.0, "description": "Treble gain in dB (high shelf, ~3 kHz): 4 brightens, -4 softens hiss/harshness. 0 leaves the band unchanged." },
                    "format": { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_eq_suffix_and_format_ext() {
        assert_eq!(filename_with_suffix("song.wav", "-eq", "mp3"), "song-eq.mp3");
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-eq", "flac"),
            "voice memo-eq.flac"
        );
    }
}
