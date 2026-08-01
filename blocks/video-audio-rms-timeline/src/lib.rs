//! gizza-ai/video-audio-rms-timeline — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI and the generated page controls); handle() delegates to
//! block_utils::run_skill.
//!
//! Pure Rust (symphonia decode + windowed level maths in
//! gizza-ai-video-audio-rms-timeline-core) → runs on every backend, including
//! the chat Service Worker. The file is supplied as base64/hex bytes so the
//! same descriptor serves chat, CLI and the page's text field.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default = "default_window_ms")]
    window_ms: f64,
    #[serde(default)]
    hop_ms: f64,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    output: String,
}

fn default_window_ms() -> f64 {
    100.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The video (or audio) file bytes encoded as base64 or hex. Its first decodable audio track is analyzed; a silent/video-only file is rejected. Containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus/AC-3/DTS are not supported)."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted file bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::number("window_ms")
                .min(1.0)
                .max(60_000.0)
                .default(100.0)
                .describe("Analysis window length in milliseconds (1–60000, default 100). Each output row measures one window; shorter windows track fast transients, longer windows smooth the level."),
        )
        .param(
            Param::number("hop_ms")
                .min(0.0)
                .max(60_000.0)
                .default(0.0)
                .describe("Step between successive window starts in milliseconds (0–60000, default 0). 0 means non-overlapping (hop equals the window). A hop smaller than the window overlaps windows for a smoother curve; larger than the window skips audio between measurements."),
        )
        .param(
            Param::enumv("unit", ["dbfs", "linear"])
                .default("dbfs")
                .describe("Level unit for the rms/peak columns: 'dbfs' (default, 0 dB = full scale; digital silence is floored to -120 dB) or 'linear' (0..1 amplitude fraction of full scale)."),
        )
        .param(
            Param::enumv("output", ["csv", "json"])
                .default("csv")
                .describe("Output format: 'csv' (default) with a header row window,start_s,end_s,rms,peak; or 'json' with per-window objects plus sample_rate, channels, duration and window metadata."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoAudioRmsTimeline;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-rms-timeline",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract windowed RMS and peak audio levels from a video into a CSV time series.",
    skill(
        description = "Extract a windowed audio-level time series from a video (or plain audio) file. Paste the file bytes as base64 or hex; the first decodable audio track is downmixed to mono and sliced into fixed-length windows, and each window's RMS (average energy) and peak (loudest sample) level is reported as a CSV (window,start_s,end_s,rms,peak) or JSON time series. Configure the window length (window_ms, default 100), the hop between windows (hop_ms, default 0 = non-overlapping; set it below the window for overlapping frames), the level unit (dbfs, 0 dB = full scale with silence floored to -120 dB; or linear 0..1 amplitude), and the output format (csv or json). Supported containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; codecs AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus/AC-3/DTS are not supported). Levels are measured on the mono downmix; a silent/video-only file is rejected. Long audio is capped at about five minutes of analysis.",
        parameters = schema_json()
    ),
)]
impl VideoAudioRmsTimeline {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "video-audio-rms-timeline", |a: Args| {
            gizza_ai_video_audio_rms_timeline_core::run(
                &a.input,
                &a.input_format,
                a.window_ms,
                a.hop_ms,
                &a.unit,
                &a.output,
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
        assert_eq!(schema.get("required").unwrap(), &serde_json::json!(["input"]));
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");
        assert_eq!(props["window_ms"]["type"], "number");
        assert_eq!(props["window_ms"]["minimum"], 1.0);
        assert_eq!(props["window_ms"]["maximum"], 60000.0);
        assert_eq!(props["window_ms"]["default"], 100.0);
        assert_eq!(props["hop_ms"]["default"], 0.0);
        assert_eq!(props["unit"]["enum"], serde_json::json!(["dbfs", "linear"]));
        assert_eq!(props["unit"]["default"], "dbfs");
        assert_eq!(props["output"]["enum"], serde_json::json!(["csv", "json"]));
        assert_eq!(props["output"]["default"], "csv");
        for key in ["input", "input_format", "window_ms", "hop_ms", "unit", "output"] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
    }
}
