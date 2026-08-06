//! Browser-facing wasm-bindgen wrapper for /tools/usnjrnl-parser/.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false" strings — parse positive-truthy.
fn flag(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

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

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    input_encoding: &str,
    event: &str,
    include: &str,
    filter: &str,
    pair_renames: &str,
    mode: &str,
    host: &str,
    sort: &str,
    max_entries: &str,
) -> Result<String, JsValue> {
    gizza_ai_usnjrnl_parser_core::run(
        data,
        input_encoding,
        event,
        include,
        filter,
        flag(pair_renames, true),
        mode,
        host,
        sort,
        usize_or(max_entries, 200),
    )
    .map_err(|e| JsValue::from_str(&e))
}
