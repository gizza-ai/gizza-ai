//! Browser-facing wasm-bindgen wrapper for /tools/base85-codec/.
//! Field order MUST match page/meta.toml: input, mode, variant, format, adobe_frame.
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    variant: &str,
    format: &str,
    adobe_frame: &str,
) -> Result<String, JsValue> {
    gizza_ai_base85_codec_core::convert(input, mode, variant, format, parse_bool(adobe_frame))
        .map_err(|e| JsValue::from_str(&e))
}
