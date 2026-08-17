//! Browser-facing wasm-bindgen wrapper for /tools/sphere-to-wav/.
//! Page fields arrive as strings; the two numeric ones parse with their
//! defaults so a blank box means "from the start" / "to the end".
//!
//! Field ORDER in page/meta.toml MUST match this parameter order.
use wasm_bindgen::prelude::*;

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
    encoding: &str,
    channel: &str,
    container: &str,
    byte_order: &str,
    start_sample: &str,
    max_samples: &str,
) -> Result<String, JsValue> {
    gizza_ai_sphere_to_wav_core::run(
        input,
        input_format,
        output,
        encoding,
        channel,
        container,
        byte_order,
        parse_u64(start_sample, 0),
        parse_u64(max_samples, 0),
    )
    .map_err(|e| JsValue::from_str(&e))
}
