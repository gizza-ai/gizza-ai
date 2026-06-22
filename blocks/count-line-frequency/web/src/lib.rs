//! Browser-facing wasm-bindgen wrapper for /tools/count-line-frequency/.
//! Field order MUST match meta.toml: text, case_sensitive, trim.
use gizza_ai_count_line_frequency_core::format_table;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no" | "off" | "")
}

#[wasm_bindgen]
pub fn run(text: &str, case_sensitive: &str, trim: &str) -> Result<String, JsValue> {
    Ok(format_table(text, truthy(case_sensitive), truthy(trim)))
}
