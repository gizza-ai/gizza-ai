//! Browser-facing wasm-bindgen wrapper for /tools/net-worth-tracker/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, sort: &str, currency: &str) -> Result<String, JsValue> {
    let sort = gizza_ai_net_worth_tracker_core::parse_sort(sort).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_net_worth_tracker_core::format_report(input, sort, currency)
        .map_err(|e| JsValue::from_str(&e))
}
