//! Browser-facing wasm-bindgen wrapper for /tools/amcache-parser/.
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

#[wasm_bindgen]
pub fn run(
    data: &str,
    input_encoding: &str,
    section: &str,
    mode: &str,
    association: &str,
    filter: &str,
    sort: &str,
    max_entries: &str,
) -> Result<String, JsValue> {
    gizza_ai_amcache_parser_core::run(
        data,
        input_encoding,
        section,
        mode,
        association,
        filter,
        sort,
        usize_or(max_entries, 200),
    )
    .map_err(|e| JsValue::from_str(&e))
}
