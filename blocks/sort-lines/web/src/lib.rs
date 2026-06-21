//! Browser-facing wasm-bindgen wrapper for /tools/sort-lines/.
//! Field order MUST match meta.toml: text, method, order, ignore_case, trim,
//! unique, remove_blank. Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_sort_lines_core::{render, Method, Order};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    method: &str,
    order: &str,
    ignore_case: &str,
    trim: &str,
    unique: &str,
    remove_blank: &str,
) -> Result<String, JsValue> {
    let m = Method::parse(method).map_err(|e| JsValue::from_str(&e))?;
    let o = Order::parse(order).map_err(|e| JsValue::from_str(&e))?;
    render(
        text,
        m,
        o,
        truthy(ignore_case),
        truthy(trim),
        truthy(unique),
        truthy(remove_blank),
    )
    .map_err(|e| JsValue::from_str(&e))
}
