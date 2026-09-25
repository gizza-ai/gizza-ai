//! Browser-facing wasm-bindgen wrapper for /tools/js-linter/.
//! Field order MUST match page/meta.toml.
use gizza_ai_js_linter_core::run_with_options;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    code: &str,
    preset: &str,
    ecma: &str,
    env: &str,
    source_type: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, JsValue> {
    run_with_options(
        code,
        preset,
        ecma,
        env,
        source_type,
        min_severity,
        ignore,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
