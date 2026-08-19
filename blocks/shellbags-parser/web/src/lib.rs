//! Browser-facing wasm-bindgen wrapper for /tools/shellbags-parser/.
//! Field values arrive as strings (checkboxes as "true"/"false"), so numbers are
//! parsed leniently and an empty field falls back to the descriptor default.
use wasm_bindgen::prelude::*;

fn usize_or(s: &str, default: usize) -> usize {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| *v >= 0.0)
        .map(|v| v as usize)
        .unwrap_or(default)
}

fn truthy(s: &str) -> bool {
    matches!(s.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    input_encoding: &str,
    mode: &str,
    bag_root: &str,
    custom_path: &str,
    max_entries: &str,
    max_depth: &str,
    resolve_guids: &str,
) -> Result<String, JsValue> {
    gizza_ai_shellbags_parser_core::run(
        data,
        input_encoding,
        mode,
        bag_root,
        custom_path,
        usize_or(max_entries, 200),
        usize_or(max_depth, 32),
        truthy(resolve_guids),
    )
    .map_err(|e| JsValue::from_str(&e))
}
