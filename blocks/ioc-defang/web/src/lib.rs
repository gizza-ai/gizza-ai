//! Browser-facing wasm-bindgen wrapper for /tools/ioc-defang/.
//! Compiled with wasm-pack for the standalone /tools/ioc-defang/ page.
use wasm_bindgen::prelude::*;

/// Defang (default) or refang the IOCs in `text`. The page passes the two
/// select values as strings; blank `mode` → defang, blank `style` → square.
/// Throws a JS error string on an unknown mode/style.
#[wasm_bindgen]
pub fn run(text: &str, mode: &str, style: &str) -> Result<String, JsValue> {
    gizza_ai_ioc_defang_core::defang(text, mode, style).map_err(|e| JsValue::from_str(&e))
}
