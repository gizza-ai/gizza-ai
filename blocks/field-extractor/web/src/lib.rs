//! Browser-facing wasm-bindgen wrapper for /tools/field-extractor/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    mode: &str,
    selectors: &str,
    delimiter: &str,
    output_delimiter: &str,
    trim: &str,
    skip_empty_lines: &str,
    skip_header: &str,
) -> Result<String, JsValue> {
    gizza_ai_field_extractor_core::extract(
        text,
        empty_default(mode, "fields"),
        selectors,
        delimiter,
        output_delimiter,
        truthy(trim),
        truthy(skip_empty_lines),
        truthy(skip_header),
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
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
