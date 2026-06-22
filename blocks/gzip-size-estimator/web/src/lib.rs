//! Browser-facing wasm-bindgen wrapper for /tools/gzip-size-estimator/.
//! Compiled with wasm-pack for the standalone /tools/gzip-size-estimator/ page.
use wasm_bindgen::prelude::*;

/// Report the raw and gzipped byte size of `input`.
///
/// The standalone tool page passes every field value as a string, so `level`
/// arrives as a string and is parsed here (blank/unparseable → the default 6;
/// the core clamps the 0–9 range). Throws a JS error string on empty input.
#[wasm_bindgen]
pub fn run(input: &str, level: &str) -> Result<String, JsValue> {
    let level = level
        .trim()
        .parse::<u32>()
        .unwrap_or(gizza_ai_gzip_size_estimator_core::DEFAULT_LEVEL);
    gizza_ai_gzip_size_estimator_core::estimate(input, level).map_err(|e| JsValue::from_str(&e))
}
