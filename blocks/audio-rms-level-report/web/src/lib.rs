//! Browser-facing wasm-bindgen wrapper for /tools/audio-rms-level-report/.
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
    output: &str,
    rms_window_ms: &str,
    clip_threshold: &str,
) -> Result<String, JsValue> {
    gizza_ai_audio_rms_level_report_core::run(
        input,
        input_format,
        output,
        parse_f64(rms_window_ms, 50.0),
        parse_f64(clip_threshold, 0.99),
    )
    .map_err(|e| JsValue::from_str(&e))
}
