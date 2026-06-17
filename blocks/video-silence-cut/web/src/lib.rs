//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-silence-cut/
//! page (ffmpeg). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! NOTE: this is a single-pass **approximation** of silence-cutting — the audio is
//! de-silenced but the video drifts out of sync. See the core module docs and the PR.
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Plan {
    argv: Vec<String>,
    out_name: String,
}

/// `threshold_db` (dB, e.g. -30 — quieter than this counts as silence) and
/// `min_silence` (seconds — gaps at least this long get trimmed). The page passes
/// the field values in this order, then the input filename. Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(threshold_db: f64, min_silence: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_silence_cut_core::plan(in_name, threshold_db, min_silence)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
