//! Browser-facing wasm-bindgen wrapper for /tools/markdown-link-flattener/.
use wasm_bindgen::prelude::*;

fn flag(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    link_mode: &str,
    image_mode: &str,
    reference_definitions: &str,
    preserve_code: &str,
) -> Result<String, JsValue> {
    gizza_ai_markdown_link_flattener_core::run(
        markdown,
        link_mode,
        image_mode,
        reference_definitions,
        flag(preserve_code, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
