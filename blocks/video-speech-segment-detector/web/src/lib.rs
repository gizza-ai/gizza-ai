//! Browser-facing wasm-bindgen wrapper for /tools/video-speech-segment-detector/.
//!
//! The page is a DETECT-then-REPORT flow (like the chat/CLI block), driven by
//! `page/custom.js` (video-autocrop-bars takeover shape):
//!   1. `detect_argv(threshold_db, min_silence, voice_band, in_name)` →
//!      silencedetect plan (no output file; the page runs it and keeps the
//!      ffmpeg LOG);
//!   2. `segments_report(log, min_speech, pad, segments, output)` → the
//!      rendered text + summary numbers + download filename/mime.
//! All parsing/segmentation/rendering logic is shared with the chat block via
//! `core`. `error_message(log)` maps the one common failure (no audio track)
//! to a clear message for the page's error path.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_speech_segment_detector_core as core;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Positive-truthy checkbox marshaling ("true"/"1"/"on"/"yes" → true) — the
/// page sends the CHECKED state as a string via readField.
fn truthy(v: &str) -> bool {
    matches!(v, "true" | "1" | "on" | "yes")
}

/// Pass-1 plan. NaN numeric fields (empty inputs) fall back to the defaults;
/// field order matches page/meta.toml.
#[wasm_bindgen]
pub fn detect_argv(
    threshold_db: f64,
    min_silence: f64,
    voice_band: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let threshold_db = if threshold_db.is_nan() {
        core::DEFAULT_THRESHOLD_DB
    } else {
        threshold_db
    };
    let min_silence = if min_silence.is_nan() {
        core::DEFAULT_MIN_SILENCE_S
    } else {
        min_silence
    };
    core::validate(threshold_db, min_silence, 0.0, 0.0).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan {
        argv: core::detect_argv(in_name, threshold_db, min_silence, truthy(voice_band)),
        out_name: "detect.null".into(),
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
struct Report {
    text: String,
    filename: String,
    mime: String,
    duration_seconds: f64,
    speech_segments: usize,
    non_speech_segments: usize,
    speech_seconds: f64,
    speech_percent: f64,
}

/// Pass-2: parse the detect pass's ffmpeg log and render the report. Errors
/// (unreadable log, bad enum values) come back as strings the page shows
/// verbatim.
#[wasm_bindgen]
pub fn segments_report(
    log: &str,
    min_speech: f64,
    pad: f64,
    segments: &str,
    output: &str,
) -> Result<JsValue, JsValue> {
    let min_speech = if min_speech.is_nan() {
        core::DEFAULT_MIN_SPEECH_S
    } else {
        min_speech
    };
    let pad = if pad.is_nan() { core::DEFAULT_PAD_S } else { pad };
    core::validate(core::DEFAULT_THRESHOLD_DB, core::DEFAULT_MIN_SILENCE_S, min_speech, pad)
        .map_err(|e| JsValue::from_str(&e))?;
    let filter = core::SegmentFilter::parse(if segments.is_empty() { "both" } else { segments })
        .map_err(|e| JsValue::from_str(&e))?;
    let format = core::OutputFormat::parse(if output.is_empty() { "report" } else { output })
        .map_err(|e| JsValue::from_str(&e))?;
    let (duration, silences) = core::parse_log(log).map_err(|e| JsValue::from_str(&e))?;
    let segs = core::segments(duration, &silences, min_speech, pad);
    let speech_seconds = core::speech_seconds(&segs);
    let round3 = |v: f64| (v * 1000.0).round() / 1000.0;
    let report = Report {
        text: core::render(&segs, duration, filter, format),
        filename: format!("speech-segments.{}", format.extension()),
        mime: format.mime().to_string(),
        duration_seconds: round3(duration),
        speech_segments: segs.iter().filter(|s| s.speech).count(),
        non_speech_segments: segs.iter().filter(|s| !s.speech).count(),
        speech_seconds: round3(speech_seconds),
        speech_percent: if duration > 0.0 {
            round3(100.0 * speech_seconds / duration)
        } else {
            0.0
        },
    };
    serde_wasm_bindgen::to_value(&report).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Friendly message for a failed detect pass: the common no-audio-track case
/// gets a clear explanation; anything else surfaces the log's last line.
#[wasm_bindgen]
pub fn error_message(log: &str) -> String {
    if core::no_audio_error(log) {
        return "the file has no audio track to analyze — speech detection needs an audio stream"
            .into();
    }
    log.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("ffmpeg failed")
        .to_string()
}
