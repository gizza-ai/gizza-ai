//! Browser-facing wasm-bindgen wrapper for /tools/spectral-eq-match/.
//! The page driver hands every field through as a raw string, so each numeric
//! and boolean param is parsed here and the core owns all validation.
use wasm_bindgen::prelude::*;

fn num(name: &str, raw: &str, default: f64) -> Result<f64, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
}

fn flag(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    track: &str,
    reference: &str,
    target_curve: &str,
    amount: &str,
    max_gain_db: &str,
    smoothing: &str,
    q: &str,
    tone_only: &str,
    track_lufs: &str,
    target_lufs: &str,
    output: &str,
) -> Result<String, JsValue> {
    let amount = num("amount", amount, 100.0)?;
    let max_gain_db = num("max_gain_db", max_gain_db, 6.0)?;
    let smoothing_f = num("smoothing", smoothing, 1.0)?;
    if !(0.0..=4.0).contains(&smoothing_f) {
        return Err(JsValue::from_str(
            "smoothing is out of range (expected 0-4 neighbouring bands)",
        ));
    }
    let q = num("q", q, 1.0)?;
    let track_lufs = num("track_lufs", track_lufs, 0.0)?;
    let target_lufs = num("target_lufs", target_lufs, 0.0)?;
    gizza_ai_spectral_eq_match_core::match_eq(
        track,
        reference,
        target_curve,
        amount,
        max_gain_db,
        smoothing_f.round() as u32,
        q,
        flag(tone_only, true),
        track_lufs,
        target_lufs,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
