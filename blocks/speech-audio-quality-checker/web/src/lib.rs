//! Browser-facing wasm-bindgen wrapper for /tools/speech-audio-quality-checker/.
use wasm_bindgen::prelude::*;

fn parse_u32(s: &str, default: u32) -> u32 {
    let trimmed = s.trim();
    if trimmed.is_empty() { default } else { trimmed.parse().unwrap_or(default) }
}

fn parse_f64(s: &str, default: f64) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() { default } else { trimmed.parse().unwrap_or(default) }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    target_sample_rate: &str,
    min_snr_db: &str,
    max_clipping_pct: &str,
    clipping_threshold: &str,
) -> Result<String, JsValue> {
    gizza_ai_speech_audio_quality_checker_core::run(
        input,
        input_format,
        output,
        parse_u32(target_sample_rate, 16_000),
        parse_f64(min_snr_db, 20.0),
        parse_f64(max_clipping_pct, 1.0),
        parse_f64(clipping_threshold, 0.99),
    )
    .map_err(|e| JsValue::from_str(&e))
}
