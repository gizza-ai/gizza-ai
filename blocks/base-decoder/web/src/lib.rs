//! Browser-facing wasm-bindgen wrapper for /tools/base-decoder/.
//! Field order MUST match page/meta.toml: input, max_depth, output.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, max_depth: &str, output: &str) -> Result<String, JsValue> {
    let depth = max_depth
        .trim()
        .parse::<usize>()
        .unwrap_or(gizza_ai_base_decoder_core::DEFAULT_DEPTH);
    gizza_ai_base_decoder_core::decode(input, depth, output).map_err(|e| JsValue::from_str(&e))
}
