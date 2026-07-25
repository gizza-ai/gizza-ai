//! Browser-facing wasm-bindgen wrapper for /tools/json-sort/.
//! Field order MUST match meta.toml: json, order, sort_arrays, case_insensitive, indent.
//! Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_json_sort_core::{sort, Options, Order};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    order: &str,
    sort_arrays: &str,
    case_insensitive: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        order: Order::parse(order),
        sort_arrays: truthy(sort_arrays),
        case_insensitive: truthy(case_insensitive),
        indent: indent.trim().parse().unwrap_or(2),
    };
    sort(json, opts).map_err(|e| JsValue::from_str(&e))
}
