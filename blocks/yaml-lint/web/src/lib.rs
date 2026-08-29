//! Browser-facing wasm-bindgen wrapper for /tools/yaml-lint/.
//! Field order MUST match meta.toml. Fields arrive as strings from the generated page.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    preset: &str,
    indent_spaces: &str,
    max_line_length: &str,
    disable: &str,
    strict_warnings: &str,
    report_format: &str,
) -> Result<String, JsValue> {
    gizza_ai_yaml_lint_core::run_str(
        input,
        preset,
        indent_spaces,
        max_line_length,
        disable,
        strict_warnings,
        report_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
