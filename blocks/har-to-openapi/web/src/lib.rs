//! Browser-facing wasm-bindgen wrapper for /tools/har-to-openapi/.
//! Field order MUST match meta.toml: har, format, openapi_version,
//! parameterize_paths, infer_types, include_examples, domain, title,
//! drop_unsuccessful.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    har: &str,
    format: &str,
    openapi_version: &str,
    parameterize_paths: &str,
    infer_types: &str,
    include_examples: &str,
    domain: &str,
    title: &str,
    drop_unsuccessful: &str,
) -> Result<String, JsValue> {
    gizza_ai_har_to_openapi_core::run(
        har,
        format,
        openapi_version,
        truthy(parameterize_paths),
        truthy(infer_types),
        truthy(include_examples),
        domain,
        title,
        truthy(drop_unsuccessful),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
