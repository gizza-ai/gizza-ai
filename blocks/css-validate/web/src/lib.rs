//! Browser-facing wasm-bindgen wrapper for /tools/css-validate/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    css: &str,
    format: &str,
    severity: &str,
    unknown_properties: &str,
    vendor_prefixes: &str,
    stats: &str,
) -> Result<String, JsValue> {
    gizza_ai_css_validate_core::run(
        css,
        format,
        severity,
        unknown_properties,
        vendor_prefixes,
        stats,
    )
    .map_err(|e| JsValue::from_str(&e))
}
