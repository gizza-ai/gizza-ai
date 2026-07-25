//! Browser-facing wasm-bindgen wrapper for /tools/python-dict-to-json/.
//! Compiled with wasm-pack for the standalone /tools/python-dict-to-json/ page.
use wasm_bindgen::prelude::*;

/// Convert a Python dict/list literal in `input` to JSON.
///
/// The standalone tool page passes every field value as a string, so the boolean
/// params arrive as strings and are parsed here:
/// - `indent`: `"2"`/`"4"`/`"tab"`/`"minify"` (blank → 2 spaces).
/// - `sort_keys`: `"true"`/`"1"`/`"yes"`/`"on"` → alphabetize keys; else off.
/// - `ensure_ascii`: `"true"`/`"1"`/`"yes"`/`"on"` → escape non-ASCII; else off.
///
/// Throws a JS error string on a malformed Python literal.
#[wasm_bindgen]
pub fn run(input: &str, indent: &str, sort_keys: &str, ensure_ascii: &str) -> Result<String, JsValue> {
    let sort_keys = truthy(sort_keys);
    let ensure_ascii = truthy(ensure_ascii);
    gizza_ai_python_dict_to_json_core::convert(input, indent, sort_keys, ensure_ascii)
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
