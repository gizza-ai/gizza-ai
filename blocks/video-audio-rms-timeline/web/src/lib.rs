//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-rms-timeline/.
//! Every field arrives from the page as a string; parse the numerics with the
//! same defaults the descriptor declares (blank → default).
use wasm_bindgen::prelude::*;

fn parse_f64(s: &str, default: f64) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed.parse().unwrap_or(default)
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    window_ms: &str,
    hop_ms: &str,
    unit: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_video_audio_rms_timeline_core::run(
        input,
        input_format,
        parse_f64(window_ms, 100.0),
        parse_f64(hop_ms, 0.0),
        unit,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
