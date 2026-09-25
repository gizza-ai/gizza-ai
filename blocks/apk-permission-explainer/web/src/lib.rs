//! Browser-facing wasm-bindgen wrapper for /tools/apk-permission-explainer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, mode: &str, risk: &str, sort: &str) -> Result<String, JsValue> {
    gizza_ai_apk_permission_explainer_core::run(
        input,
        empty_default(mode, "report"),
        empty_default(risk, "all"),
        empty_default(sort, "risk"),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn empty_default<'a>(s: &'a str, d: &'a str) -> &'a str {
    if s.trim().is_empty() {
        d
    } else {
        s
    }
}
