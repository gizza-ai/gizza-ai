//! Browser-facing wasm-bindgen wrapper for /tools/mock-data-generator/.
//! Compiled with wasm-pack for the standalone /tools/mock-data-generator/ page.
//!
//! The page passes every field value as a string, so the integer/boolean params
//! arrive as strings and are parsed here:
//! - `schema`: the shorthand schema text (required).
//! - `count`: a record count 1-1000 (blank/unparseable → 1; the core clamps/validates).
//! - `seed`: a reproducibility seed (blank/0 → derive a varying seed from the
//!   browser clock via js_sys::Date so each run differs; non-zero reproduces).
//! - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → indented; anything else → compact.
//!
//! Throws a JS error string on an invalid schema or out-of-range count.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(schema: &str, count: &str, seed: &str, pretty: &str) -> Result<String, JsValue> {
    let count = count.trim().parse::<u32>().unwrap_or(1);
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let mut seed = seed.trim().parse::<u64>().unwrap_or(0);
    if seed == 0 {
        // No std clock on wasm32-unknown-unknown — use the browser clock so
        // repeated generations differ; a non-zero seed always reproduces.
        seed = (js_sys::Date::now() as u64).max(1);
    }
    gizza_ai_mock_data_generator_core::generate(schema, count, seed, pretty)
        .map_err(|e| JsValue::from_str(&e))
}
