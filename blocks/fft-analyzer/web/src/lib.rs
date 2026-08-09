//! Browser-facing wasm-bindgen wrapper for /tools/fft-analyzer/.
//!
//! The page driver passes every field as a STRING (`gatherArgs` reads raw input
//! values), so numeric params are parsed here and fall back to the descriptor
//! default when the field is left empty — an empty box must not read as 0.
use wasm_bindgen::prelude::*;

fn number(name: &str, raw: &str, default: f64) -> Result<f64, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{name} must be a number (got '{t}')"))
}

fn integer(name: &str, raw: &str, default: i64) -> Result<i64, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .or_else(|_| t.parse::<f64>().map(|v| v.round() as i64))
        .map_err(|_| format!("{name} must be a whole number (got '{t}')"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    sample_rate: &str,
    window: &str,
    pad: &str,
    spectrum: &str,
    scale: &str,
    phase_unit: &str,
    remove_dc: &str,
    peaks: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let go = || -> Result<String, String> {
        let rate = number("sample_rate", sample_rate, 1.0)?;
        let peaks = integer("peaks", peaks, 5)?;
        let decimals = integer("decimals", decimals, 4)?;
        let remove_dc = matches!(
            remove_dc.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        );
        gizza_ai_fft_analyzer_core::analyze(
            data, rate, window, pad, spectrum, scale, phase_unit, remove_dc, peaks, decimals,
            format,
        )
    };
    go().map_err(|e| JsValue::from_str(&e))
}
