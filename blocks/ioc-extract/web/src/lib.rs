//! Browser-facing wasm-bindgen wrapper for /tools/ioc-extract/.
//! Compiled with wasm-pack for the standalone /tools/ioc-extract/ page.
use wasm_bindgen::prelude::*;

/// Extract IOCs from `text`. `types` is the (blank → "all") category selector;
/// `defang` is the checkbox value as a string ("true"/"on"/"1" → re-defang the
/// output). Throws a JS error string on an unknown type.
#[wasm_bindgen]
pub fn run(text: &str, types: &str, defang: &str) -> Result<String, JsValue> {
    let defang = matches!(
        defang.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_ioc_extract_core::extract(text, types, defang).map_err(|e| JsValue::from_str(&e))
}
