//! Browser-facing wasm-bindgen wrapper for /tools/wrap-lines-in-quotes/.
//! Field order MUST match meta.toml: text, wrap, open, close, separator,
//! last_line_separator, skip_empty, trim, escape. Fields arrive as strings
//! (checkboxes send "true"/"false").
use gizza_ai_wrap_lines_in_quotes_core::render;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    wrap: &str,
    open: &str,
    close: &str,
    separator: &str,
    last_line_separator: &str,
    skip_empty: &str,
    trim: &str,
    escape: &str,
) -> Result<String, JsValue> {
    // An empty wrap field falls back to the schema default (double quotes).
    let wrap = if wrap.is_empty() { "double" } else { wrap };
    render(
        text,
        wrap,
        open,
        close,
        separator,
        truthy(last_line_separator),
        truthy(skip_empty),
        truthy(trim),
        truthy(escape),
    )
    .map_err(|e| JsValue::from_str(&e))
}
