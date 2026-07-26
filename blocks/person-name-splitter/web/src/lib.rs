//! Browser-facing wasm-bindgen wrapper for /tools/person-name-splitter/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    name_column: &str,
    output: &str,
    delimiter: &str,
    header: &str,
    trim: &str,
) -> Result<String, JsValue> {
    gizza_ai_person_name_splitter_core::run(
        data,
        name_column,
        if output.trim().is_empty() {
            "append"
        } else {
            output
        },
        truthy(header, true),
        if delimiter.trim().is_empty() {
            "comma"
        } else {
            delimiter
        },
        truthy(trim, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
