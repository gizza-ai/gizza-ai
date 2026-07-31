//! Browser-facing wasm-bindgen wrapper for /tools/sort-json-array/.
//! Field order MUST match meta.toml: json, keys, order, missing, case_insensitive, indent.
//! Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_sort_json_array_core::{sort, Missing, Options, Order};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    keys: &str,
    order: &str,
    missing: &str,
    case_insensitive: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        keys: keys.to_string(),
        order: Order::parse(order),
        missing: Missing::parse(missing),
        case_insensitive: truthy(case_insensitive),
        indent: indent.trim().parse().unwrap_or(2),
    };
    sort(json, &opts).map_err(|e| JsValue::from_str(&e))
}
