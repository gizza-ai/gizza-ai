//! Browser-facing wasm-bindgen wrapper for /tools/html-to-jsx/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    html: &str,
    indent: &str,
    component: &str,
    comments: &str,
    boolean_attrs: &str,
    value_attrs: &str,
) -> Result<String, JsValue> {
    gizza_ai_html_to_jsx_core::html_to_jsx(
        html,
        indent,
        component,
        comments,
        boolean_attrs,
        value_attrs,
    )
    .map_err(|e| JsValue::from_str(&e))
}
