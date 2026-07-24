//! Browser-facing wasm-bindgen wrapper for /tools/yaml-to-csv/.
//! Field order MUST match meta.toml: data, delimiter, header, array_mode,
//! quote_all, key_column. Checkboxes arrive as "true"/"false" strings.
use gizza_ai_yaml_to_csv_core::to_csv;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    array_mode: &str,
    quote_all: &str,
    key_column: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    let amode = if array_mode.is_empty() { "json" } else { array_mode };
    to_csv(data, delim, truthy(header), amode, truthy(quote_all), key_column)
        .map_err(|e| JsValue::from_str(&e))
}
