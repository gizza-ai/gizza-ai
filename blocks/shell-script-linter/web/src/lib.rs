//! Browser-facing wasm-bindgen wrapper for /tools/shell-script-linter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    script: &str,
    shell: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_shell_script_linter_core::lint(script, shell, min_severity, ignore, format)
        .map_err(|e| JsValue::from_str(&e))
}
