//! Browser-facing wasm-bindgen wrapper for /tools/one-time-pad/.
//! Field order MUST match meta.toml: mode, cipher, message, pad, encoding,
//! length, group. Numeric fields arrive as f64 (blank = NaN), so they are
//! normalized to the schema defaults here.
use wasm_bindgen::prelude::*;

/// A blank/NaN numeric field falls back to 0; anything else must be a whole
/// number within `max`.
fn whole(v: f64, name: &str, max: u64) -> Result<usize, String> {
    if !v.is_finite() {
        return Ok(0);
    }
    if v < 0.0 || v.fract() != 0.0 {
        return Err(format!("{name} must be a whole number 0 or more (got {v})"));
    }
    let n = v as u64;
    if n > max {
        return Err(format!("{name} must be at most {max} (got {n})"));
    }
    Ok(n as usize)
}

#[wasm_bindgen]
pub fn run(
    mode: &str,
    cipher: &str,
    message: &str,
    pad: &str,
    encoding: &str,
    length: f64,
    group: f64,
) -> Result<String, JsValue> {
    let length = whole(length, "length", 16384).map_err(|e| JsValue::from_str(&e))?;
    let group = whole(group, "group", 20).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_one_time_pad_core::run(mode, cipher, message, pad, encoding, length, group)
        .map_err(|e| JsValue::from_str(&e))
}
