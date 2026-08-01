//! Browser-facing wasm-bindgen wrapper for /tools/text-colorizer/.
//! Field values arrive as strings; checkboxes send "true"/"false" — coerce with
//! positive-truthy matching. Param order MUST equal page/meta.toml's inputs.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v, "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    rules: &str,
    output: &str,
    theme: &str,
    ignore_case: &str,
    whole_line: &str,
) -> Result<String, JsValue> {
    gizza_ai_text_colorizer_core::colorize(
        text,
        rules,
        output,
        theme,
        truthy(ignore_case),
        truthy(whole_line),
    )
    .map_err(|e| JsValue::from_str(&e))
}
