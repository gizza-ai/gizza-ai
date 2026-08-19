//! Browser-facing wasm-bindgen wrapper for /tools/wav-samples-to-json/.
//! Page fields arrive as strings; parse the numeric ones with defaults.
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

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    layout: &str,
    value_scale: &str,
    precision: &str,
    start_frame: &str,
    frame_step: &str,
    max_frames: &str,
    indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_wav_samples_to_json_core::run(
        input,
        input_format,
        output,
        layout,
        value_scale,
        parse_u32(precision, 6),
        parse_u64(start_frame, 0),
        parse_u64(frame_step, 1),
        parse_u64(max_frames, 50_000),
        parse_u32(indent, 2),
    )
    .map_err(|e| JsValue::from_str(&e))
}
