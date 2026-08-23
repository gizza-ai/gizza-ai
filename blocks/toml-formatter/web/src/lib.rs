//! Browser-facing wasm-bindgen wrapper for /tools/toml-formatter/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_usize(name: &str, value: &str) -> Result<usize, JsValue> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    indent: &str,
    sort_keys: &str,
    spacing: &str,
    array_style: &str,
    column_width: &str,
    align_values: &str,
    blank_line_before_tables: &str,
    keep_comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_toml_formatter_core::run(
        input,
        parse_usize("indent", indent)?,
        sort_keys,
        spacing,
        array_style,
        parse_usize("column_width", column_width)?,
        truthy(align_values),
        truthy(blank_line_before_tables),
        truthy(keep_comments),
    )
    .map_err(|e| JsValue::from_str(&e))
}
