//! Browser-facing wasm-bindgen wrapper for /tools/column-aligner/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    align: &str,
    column_align: &str,
    gap: &str,
    separator: &str,
    trim: &str,
) -> Result<String, JsValue> {
    gizza_ai_column_aligner_core::run(
        input,
        empty_default(delimiter, "whitespace"),
        empty_default(align, "left"),
        column_align,
        gap.trim().parse::<u32>().unwrap_or(2),
        separator,
        truthy_default_on(trim),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn empty_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() { default } else { s }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

fn truthy_default_on(s: &str) -> bool {
    if s.trim().is_empty() { true } else { truthy(s) }
}
