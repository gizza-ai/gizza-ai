//! Browser-facing wasm-bindgen wrapper for /tools/json-assertion-runner/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        return default;
    }
    matches!(s.as_str(), "true" | "1" | "yes" | "on")
}

#[wasm_bindgen]
pub fn run(
    payload: &str,
    assertions: &str,
    rules_format: &str,
    output: &str,
    case_sensitive: &str,
    stop_on_first_failure: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_assertion_runner_core::render(
        payload,
        assertions,
        rules_format,
        truthy(case_sensitive, true),
        truthy(stop_on_first_failure, false),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
