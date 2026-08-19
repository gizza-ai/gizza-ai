//! gizza-ai/de-esser — fetch an audio URL or attachment ref and tame harsh
//! sibilance (`s`/`sh`/`t` bursts) with ffmpeg's dedicated `deesser` filter.
//! Part of the audio-input family (`Input::Audio`).
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

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_de_esser_core::{
    parse_format, plan_deess, DEFAULT_AMOUNT, DEFAULT_BAND, DEFAULT_MAX_REDUCTION,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    band: Option<f64>,
    #[serde(default)]
    max_reduction: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("amount")
                .min(1.0)
                .max(100.0)
                .default(60.0)
                .describe("How hard sibilance is ducked once detected, as a percentage of the filter's range. 30 is a light polish, 60 is a normal vocal fix, 90+ is aggressive and can start to lisp. Default 60."),
        )
        .param(
            Param::number("band")
                .min(1.0)
                .max(100.0)
                .default(70.0)
                .describe("Where the sibilance crossover sits, 1-100 (relative, not Hz — ffmpeg's deesser uses a sample-rate-dependent one-pole split). HIGHER keeps the effect on the very top of the spectrum only; LOWER pulls the split down so more of the voice is treated as sibilance and the body dulls. Default 70."),
        )
        .param(
            Param::number("max_reduction")
                .min(1.0)
                .max(100.0)
                .default(50.0)
                .describe("Ceiling on how deep the ducking may go, 1-100. Higher lets the de-esser cut as far as it needs on strong esses; lower keeps the gain change subtle even when sibilance is loud. Default 50."),
        )
        .param(
            Param::enumv("mode", ["output", "ess", "input"])
                .default("output")
                .describe("What to render: output = the de-essed track (normal), ess = ONLY the sibilance being removed, so you can audition whether the band is right, input = the untouched audio for an A/B reference. Default output."),
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
struct DeEsser;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/de-esser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "De-ess audio: dynamically duck harsh sibilance on vocals without dulling the whole track",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "De-ess an audio file: tame the harsh s/sh/t sibilance that spikes in the upper band of a vocal, narration or podcast track. Uses ffmpeg's dynamic deesser filter, so the high band is ducked ONLY while sibilance is present and the rest of the voice keeps its brightness — unlike a static EQ cut (see audio-eq). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Controls: amount (1..100, how hard detected sibilance is ducked), band (1..100, where the sibilance crossover sits — relative, not Hz, because ffmpeg's deesser uses a sample-rate-dependent one-pole split; higher confines the effect to the very top, lower also dulls the body), max_reduction (1..100, ceiling on how deep the ducking goes) and mode (output = de-essed track, ess = only the removed sibilance for auditioning, input = untouched A/B reference). This is not a noise gate (see audio-noise-gate, which keys off overall level) and not spectral denoising (see audio-noise-reduce). Output is re-encoded to mp3 (192 kbps), wav, ogg, flac or m4a; embedded album art is dropped.",
        parameters = schema_json()
    ),
)]
impl DeEsser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; control/mode/format validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("de-esser")?;
    let amount = args.amount.unwrap_or(DEFAULT_AMOUNT);
    let band = args.band.unwrap_or(DEFAULT_BAND);
    let max_reduction = args.max_reduction.unwrap_or(DEFAULT_MAX_REDUCTION);
    let mode = args.mode.as_deref().unwrap_or("output");
    let format = args.format.as_deref().unwrap_or("mp3");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates controls + format).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_deess(&ffmpeg_in, amount, band, max_reduction, mode, format)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope with the chosen format's mime; spell out the applied settings
    //    so the LLM can echo what changed.
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-deessed", fmt.ext());
    let for_llm = format!(
        "de-essed {in_filename}: amount {amount}, band {band}, max reduction {max_reduction}, {mode} mode ({output_size} bytes {})",
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
    /// wording). Number-param defaults serialize as floats (`60.0`),
    /// whole-number bounds as integers.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":           { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":           { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "amount":        { "type": "number", "minimum": 1, "maximum": 100, "default": 60.0, "description": "How hard sibilance is ducked once detected, as a percentage of the filter's range. 30 is a light polish, 60 is a normal vocal fix, 90+ is aggressive and can start to lisp. Default 60." },
                    "band":          { "type": "number", "minimum": 1, "maximum": 100, "default": 70.0, "description": "Where the sibilance crossover sits, 1-100 (relative, not Hz — ffmpeg's deesser uses a sample-rate-dependent one-pole split). HIGHER keeps the effect on the very top of the spectrum only; LOWER pulls the split down so more of the voice is treated as sibilance and the body dulls. Default 70." },
                    "max_reduction": { "type": "number", "minimum": 1, "maximum": 100, "default": 50.0, "description": "Ceiling on how deep the ducking may go, 1-100. Higher lets the de-esser cut as far as it needs on strong esses; lower keeps the gain change subtle even when sibilance is loud. Default 50." },
                    "mode":          { "type": "string", "enum": ["output", "ess", "input"], "default": "output", "description": "What to render: output = the de-essed track (normal), ess = ONLY the sibilance being removed, so you can audition whether the band is right, input = the untouched audio for an A/B reference. Default output." },
                    "format":        { "type": "string", "enum": ["mp3", "wav", "ogg", "flac", "m4a"], "default": "mp3", "description": "Output audio format. Default mp3 (192 kbps)." }
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

    /// Every advertised enum value must actually plan, and every advertised
    /// default must be inside the advertised bounds.
    #[test]
    fn advertised_values_all_plan() {
        for mode in ["output", "ess", "input"] {
            for format in ["mp3", "wav", "ogg", "flac", "m4a"] {
                assert!(
                    plan_deess(
                        "in.mp3",
                        DEFAULT_AMOUNT,
                        DEFAULT_BAND,
                        DEFAULT_MAX_REDUCTION,
                        mode,
                        format
                    )
                    .is_ok(),
                    "advertised mode={mode} format={format} must plan"
                );
            }
        }
    }

    #[test]
    fn output_filename_uses_deessed_suffix_and_format_ext() {
        assert_eq!(
            filename_with_suffix("vocal.wav", "-deessed", "mp3"),
            "vocal-deessed.mp3"
        );
        assert_eq!(
            filename_with_suffix("podcast ep 12.m4a", "-deessed", "flac"),
            "podcast ep 12-deessed.flac"
        );
    }
}
