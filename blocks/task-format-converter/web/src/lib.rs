//! Browser-facing wasm-bindgen wrapper for /tools/task-format-converter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, from: &str, to: &str, pretty: &str) -> Result<String, JsValue> {
    let pretty = truthy(pretty);
    gizza_ai_task_format_converter_core::run(input, from, to, pretty)
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
