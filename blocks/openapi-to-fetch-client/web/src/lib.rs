//! Browser-facing wasm-bindgen wrapper for /tools/openapi-to-fetch-client/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    spec: &str,
    input_format: &str,
    style: &str,
    client_name: &str,
    naming: &str,
    param_style: &str,
    error_handling: &str,
    base_url: &str,
    types_module: &str,
    tags: &str,
    jsdoc: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent = match indent.trim() {
        "" => 2,
        s => s.parse::<u32>().map_err(|_| {
            JsValue::from_str(&format!("indent must be a whole number (got '{s}')"))
        })?,
    };
    gizza_ai_openapi_to_fetch_client_core::generate(
        spec,
        input_format,
        style,
        client_name,
        naming,
        param_style,
        error_handling,
        base_url,
        types_module,
        tags,
        truthy_or_default(jsdoc, true),
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Checkboxes arrive as "true"/"false"; blank means the descriptor default.
fn truthy_or_default(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}
