//! Browser-facing wasm-bindgen wrapper for /tools/hl7-v2-parse/.
use gizza_ai_hl7_v2_parse_core::run as core_run;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    // Boolean checkbox fields arrive as "true"/"false"; empty = the default (true).
    matches!(v.trim().to_ascii_lowercase().as_str(), "" | "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    output: &str,
    include_descriptions: &str,
    unescape: &str,
) -> Result<String, JsValue> {
    let out = if output.is_empty() { "json" } else { output };
    core_run(data, out, truthy(include_descriptions), truthy(unescape))
        .map_err(|e| JsValue::from_str(&e))
}
