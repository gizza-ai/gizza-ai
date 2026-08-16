//! gizza-ai/audio-rms-level-report — per-channel audio level summary.
//!
//! Thin chat-skill wrapper around `gizza-ai-audio-rms-level-report-core`. The
//! descriptor is the single source for the chat schema, the CLI, and the
//! generated page controls.
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
    #[serde(default = "default_rms_window_ms")]
    rms_window_ms: f64,
    #[serde(default = "default_clip_threshold")]
    clip_threshold: f64,
}

fn default_rms_window_ms() -> f64 {
    50.0
}
fn default_clip_threshold() -> f64 {
    0.99
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "Audio file bytes encoded as base64 or hex, e.g. 'UklGRi...' for a WAV. \
                     Decodes WAV, AIFF, CAF, FLAC, MP3, OGG/Vorbis, MP4/M4A (AAC-LC, ALAC), \
                     MKV/WebM and AAC-ADTS; the first audio track is measured. Limit 64 MiB \
                     of decoded bytes and 30,000,000 frames per channel.",
                ),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe(
                    "Encoding used for the pasted audio bytes: 'base64' (default) or 'hex'. \
                     Base64 may be standard or URL-safe and padding is optional; hex may \
                     include whitespace, ':' or '-' separators.",
                ),
        )
        .param(
            Param::enumv("output", ["json", "report", "csv"])
                .default("json")
                .describe(
                    "Output format. 'json' (default) is the machine-readable summary with a \
                     per_channel array and an overall object; 'report' is an aligned \
                     human-readable table; 'csv' is one row per channel plus an 'overall' row.",
                ),
        )
        .param(
            Param::number("rms_window_ms")
                .min(1.0)
                .max(10_000.0)
                .default(50.0)
                .describe(
                    "Short-window length in milliseconds used for the rms_peak_dbfs and \
                     rms_trough_dbfs figures — the loudest and quietest window of the clip \
                     (1-10000, default 50, matching the usual 50 ms level-meter window). Does \
                     not affect the whole-file RMS, peak or average.",
                ),
        )
        .param(
            Param::number("clip_threshold")
                .min(0.5)
                .max(1.0)
                .default(0.99)
                .describe(
                    "Absolute full-scale sample amplitude counted as clipped (0.5-1.0, default \
                     0.99, i.e. about -0.09 dBFS). Samples at or above this level are counted \
                     per channel and the longest consecutive run is reported.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioRmsLevelReport;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-rms-level-report",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report per-channel RMS, peak and average audio levels in dBFS.",
    skill(
        description = "Measure the level of an audio file and return a per-channel summary. Paste the file bytes as base64 or hex. The tool decodes WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/M4A, MKV/WebM and AAC-ADTS via a pure-Rust decoder and reports, for every channel and for the mix overall: RMS level, sample peak and average (mean absolute) level in both dBFS and linear full-scale amplitude, the loudest and quietest short-window RMS, crest factor, headroom to full scale, DC offset, zero crossings, and clipped-sample count / percentage / longest run. Stream facts (sample rate, channel layout, duration, frame and sample counts) come with it. Output as JSON, an aligned text report, or CSV. This is a whole-file summary — for levels over time use the video-audio-rms-timeline tool.",
        parameters = schema_json()
    )
)]
impl AudioRmsLevelReport {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "audio-rms-level-report", |a: Args| {
            gizza_ai_audio_rms_level_report_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.rms_window_ms,
                a.clip_threshold,
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

    /// Drift guard: the chat/CLI/page schema is generated from descriptor(), so
    /// this pins every param the surfaces depend on.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );
        assert_eq!(props["input"]["type"], "string");
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["json", "report", "csv"])
        );
        assert_eq!(props["output"]["default"], "json");
        assert_eq!(props["rms_window_ms"]["type"], "number");
        assert_eq!(props["rms_window_ms"]["minimum"], 1.0);
        assert_eq!(props["rms_window_ms"]["maximum"], 10000.0);
        assert_eq!(props["rms_window_ms"]["default"], 50.0);
        assert_eq!(props["clip_threshold"]["type"], "number");
        assert_eq!(props["clip_threshold"]["minimum"], 0.5);
        assert_eq!(props["clip_threshold"]["maximum"], 1.0);
        assert_eq!(props["clip_threshold"]["default"], 0.99);
        for key in [
            "input",
            "input_format",
            "output",
            "rms_window_ms",
            "clip_threshold",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
    }
}
