//! Browser-facing wasm-bindgen wrapper for /tools/raw-pcm-to-wav/.
//! Page fields arrive as strings; the four numeric ones parse with their
//! defaults so a blank box means "the usual value" / "from the start" / "to the
//! end" rather than an error.
//!
//! Field ORDER in page/meta.toml MUST match this parameter order.
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
    sample_rate: &str,
    channels: &str,
    bit_depth: &str,
    encoding: &str,
    byte_order: &str,
    skip_bytes: &str,
    max_frames: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_raw_pcm_to_wav_core::run(
        input,
        input_format,
        parse_u32(sample_rate, 44100),
        parse_u32(channels, 2),
        bit_depth,
        encoding,
        byte_order,
        parse_u64(skip_bytes, 0),
        parse_u64(max_frames, 0),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
