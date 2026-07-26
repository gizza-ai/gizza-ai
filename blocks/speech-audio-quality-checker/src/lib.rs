//! gizza-ai/speech-audio-quality-checker — check a pasted WAV clip for ASR readiness.
//!
//! Thin chat-skill wrapper around `gizza-ai-speech-audio-quality-checker-core`.
//! The descriptor is the single source for chat schema, CLI, and generated page controls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_target_sample_rate")]
    target_sample_rate: u32,
    #[serde(default = "default_min_snr_db")]
    min_snr_db: f64,
    #[serde(default = "default_max_clipping_pct")]
    max_clipping_pct: f64,
    #[serde(default = "default_clipping_threshold")]
    clipping_threshold: f64,
}

fn default_target_sample_rate() -> u32 {
    16_000
}
fn default_min_snr_db() -> f64 {
    20.0
}
fn default_max_clipping_pct() -> f64 {
    1.0
}
fn default_clipping_threshold() -> f64 {
    0.99
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Short uncompressed WAV audio clip bytes encoded as base64 or hex. Use a representative speech sample; compressed MP3/AAC/FLAC/Ogg files are rejected with a clear message."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding used for the pasted audio bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format. 'report' is a human-readable ASR readiness checklist; 'json' returns the measured metrics, checks, thresholds, and verdict."),
        )
        .param(
            Param::integer("target_sample_rate")
                .min(8000.0)
                .max(48000.0)
                .default(16_000)
                .describe("Desired ASR sample rate in Hz (8000-48000, default 16000). Recordings at or above this pass; 8 kHz telephone audio warns; below 8 kHz fails."),
        )
        .param(
            Param::number("min_snr_db")
                .min(0.0)
                .max(60.0)
                .default(20.0)
                .describe("Minimum estimated signal-to-noise ratio in dB for a PASS (0-60, default 20). 10 dB up to this value warns; below 10 dB fails."),
        )
        .param(
            Param::number("max_clipping_pct")
                .min(0.0)
                .max(100.0)
                .default(1.0)
                .describe("Maximum clipped-sample percentage allowed for a PASS (0-100, default 1.0). Higher values warn or fail depending on severity."),
        )
        .param(
            Param::number("clipping_threshold")
                .min(0.8)
                .max(1.0)
                .default(0.99)
                .describe("Absolute full-scale sample amplitude treated as clipped (0.8-1.0, default 0.99). Samples at or above this threshold count toward clipping percentage and run length."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpeechAudioQualityChecker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/speech-audio-quality-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check WAV speech audio readiness for ASR transcription.",
    skill(
        description = "Pre-flight a short speech recording for ASR/transcription readiness. Paste uncompressed WAV bytes as base64 or hex. The tool decodes PCM/IEEE-float WAV, reports sample rate, channel count, duration, peak and RMS dBFS, a percentile-based SNR estimate, clipped-sample percentage, longest clipped run, per-check PASS/WARN/FAIL statuses, and an overall verdict. Configure the target sample rate, minimum SNR, maximum clipping percentage, and clipping threshold. Compressed MP3/AAC/FLAC/Ogg inputs are rejected clearly rather than guessed.",
        parameters = schema_json()
    )
)]
impl SpeechAudioQualityChecker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "speech-audio-quality-checker", |a: Args| {
            gizza_ai_speech_audio_quality_checker_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.target_sample_rate,
                a.min_snr_db,
                a.max_clipping_pct,
                a.clipping_threshold,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["report", "json"])
        );
        assert_eq!(props["output"]["default"], "report");
        assert_eq!(props["target_sample_rate"]["minimum"], 8000);
        assert_eq!(props["target_sample_rate"]["maximum"], 48000);
        assert_eq!(props["target_sample_rate"]["default"], 16000);
        assert_eq!(props["min_snr_db"]["type"], "number");
        assert_eq!(props["min_snr_db"]["default"], 20.0);
        assert_eq!(props["max_clipping_pct"]["default"], 1.0);
        assert_eq!(props["clipping_threshold"]["minimum"], 0.8);
        assert_eq!(props["clipping_threshold"]["maximum"], 1.0);
        for key in [
            "input",
            "input_format",
            "output",
            "target_sample_rate",
            "min_snr_db",
            "max_clipping_pct",
            "clipping_threshold",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
    }
}
