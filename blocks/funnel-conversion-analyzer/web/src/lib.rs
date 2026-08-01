//! Browser-facing wasm-bindgen wrapper for /tools/funnel-conversion-analyzer/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    steps: &str,
    user: &str,
    event: &str,
    time: &str,
    ordered: &str,
    header: &str,
    delimiter: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_funnel_conversion_analyzer_core::analyze(
        data,
        steps,
        user,
        event,
        time,
        truthy(ordered, true),
        truthy(header, true),
        delimiter,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
