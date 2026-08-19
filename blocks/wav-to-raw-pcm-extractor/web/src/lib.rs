//! Browser-facing wasm-bindgen wrapper for /tools/wav-to-raw-pcm-extractor/.
//! Page fields arrive as strings; parse the numeric ones with their defaults.
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
    sample_format: &str,
    channels: &str,
    start_frame: &str,
    max_frames: &str,
    line_bytes: &str,
) -> Result<String, JsValue> {
    gizza_ai_wav_to_raw_pcm_extractor_core::run(
        input,
        input_format,
        output,
        sample_format,
        channels,
        parse_u64(start_frame, 0),
        parse_u64(max_frames, 0),
        parse_u32(line_bytes, 16),
    )
    .map_err(|e| JsValue::from_str(&e))
}
