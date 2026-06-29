//! Browser-facing wasm-bindgen wrapper for /tools/seo-audit/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str) -> Result<String, JsValue> {
    gizza_ai_seo_audit_core::audit(html).map_err(|e| JsValue::from_str(&e))
}
