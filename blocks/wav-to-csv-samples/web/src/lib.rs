//! Browser-facing wasm-bindgen wrapper for /tools/wav-to-csv-samples/.
//! Page fields arrive as strings; parse the numeric/boolean ones with defaults.
use wasm_bindgen::prelude::*;

fn parse_u32(s: &str, default: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

fn parse_u64(s: &str, default: u64) -> u64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    value_scale: &str,
    precision: &str,
    index_column: &str,
    delimiter: &str,
    header: &str,
    start_frame: &str,
    max_frames: &str,
) -> Result<String, JsValue> {
    // A boolean checkbox arrives as "true"/"false"/"on"/"1"; default (empty) → true.
    let header = matches!(header.trim(), "" | "true" | "1" | "on" | "yes");
    gizza_ai_wav_to_csv_samples_core::run(
        input,
        input_format,
        value_scale,
        parse_u32(precision, 6),
        index_column,
        delimiter,
        header,
        parse_u64(start_frame, 0),
        parse_u64(max_frames, 100_000),
    )
    .map_err(|e| JsValue::from_str(&e))
}
