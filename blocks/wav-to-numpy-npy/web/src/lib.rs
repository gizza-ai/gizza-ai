//! Browser-facing wasm-bindgen wrapper for /tools/wav-to-numpy-npy/.
//! Page fields arrive as strings; parse the numeric/boolean ones with defaults.
use wasm_bindgen::prelude::*;

fn parse_u64(s: &str, default: u64) -> u64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

/// A checkbox arrives as "true"/"false"/"on"/"1"; both booleans here default to
/// FALSE, so an empty value (field absent) must stay false.
fn parse_bool(s: &str) -> bool {
    matches!(s.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    dtype: &str,
    shape: &str,
    mono: &str,
    fortran_order: &str,
    start_frame: &str,
    max_frames: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_wav_to_numpy_npy_core::run(
        input,
        input_format,
        dtype,
        shape,
        parse_bool(mono),
        parse_bool(fortran_order),
        parse_u64(start_frame, 0),
        parse_u64(max_frames, 0),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
