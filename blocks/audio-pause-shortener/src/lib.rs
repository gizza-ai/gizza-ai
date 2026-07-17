//! gizza-ai/audio-pause-shortener — fetch an audio URL or attachment ref and
//! shorten over-long pauses down to a fixed maximum (single-pass
//! `silenceremove`, STOP side only). Part of the audio-input family
//! (`Input::Audio`). Unlike audio-silence-remove (which strips all gaps to a
//! fixed 0.25 s beat and trims leading dead air), this caps only the long
//! pauses to `target_pause`, leaving short pauses and leading silence intact.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Threshold/duration
//! validation and the pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard + unit tests
// below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_audio_pause_shortener_core::{
    parse_format, plan_pause_shorten, DEFAULT_MAX_PAUSE_S, DEFAULT_TARGET_PAUSE_S,
    DEFAULT_THRESHOLD_DB,
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

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    max_pause: Option<f64>,
    #[serde(default)]
    target_pause: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("threshold_db")
                .max(0.0)
                .describe("Silence threshold in dB (default -30). Audio quieter than this counts as a pause."),
        )
        .param(
            Param::number("max_pause")
                .min(0.0)
                .describe("Only pauses longer than this many seconds are shortened (default 1.5). Shorter pauses are left untouched."),
        )
        .param(
            Param::number("target_pause")
                .min(0.0)
                .describe("Each over-long pause is collapsed to this many seconds (default 0.5). Must be less than max_pause."),
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
struct AudioPauseShortener;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-pause-shortener",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shorten over-long pauses in a recording down to a fixed maximum",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Tighten the pacing of a speech recording by shortening over-long pauses (not removing them) with ffmpeg's silenceremove filter. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Audio quieter than threshold_db (default -30) counts as a pause; only pauses longer than max_pause seconds (default 1.5) are shortened, and each is collapsed to target_pause seconds (default 0.5, must be less than max_pause). Short pauses and leading silence are left untouched. Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a.",
        parameters = schema_json()
    ),
)]
impl AudioPauseShortener {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; threshold/duration/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-pause-shortener")?;
    let threshold_db = args.threshold_db.unwrap_or(DEFAULT_THRESHOLD_DB);
    let max_pause = args.max_pause.unwrap_or(DEFAULT_MAX_PAUSE_S);
    let target_pause = args.target_pause.unwrap_or(DEFAULT_TARGET_PAUSE_S);
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates threshold + durations + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan_pause_shorten(&ffmpeg_in, threshold_db, max_pause, target_pause, format)
            .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-tightened", fmt.ext());
    let for_llm = format!(
        "shortened pauses longer than {max_pause}s (quieter than {threshold_db} dB) down to {target_pause}s in {in_filename} ({output_size} bytes {})",
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. threshold_db /
    /// max_pause / target_pause carry no schema default — the defaults live in
    /// run() and the descriptions.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":          { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold_db": { "type": "number", "maximum": 0, "description": "Silence threshold in dB (default -30). Audio quieter than this counts as a pause." },
                    "max_pause":    { "type": "number", "minimum": 0, "description": "Only pauses longer than this many seconds are shortened (default 1.5). Shorter pauses are left untouched." },
                    "target_pause": { "type": "number", "minimum": 0, "description": "Each over-long pause is collapsed to this many seconds (default 0.5). Must be less than max_pause." },
                    "format":       { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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
    fn output_filename_uses_tightened_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("interview.wav", "-tightened", "mp3"),
            "interview-tightened.mp3"
        );
    }
}
