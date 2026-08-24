//! Browser-facing wasm-bindgen wrapper for /tools/openapi-to-curl/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    spec: &str,
    input_format: &str,
    base_url: &str,
    auth: &str,
    auth_value: &str,
    methods: &str,
    tags: &str,
    path_filter: &str,
    include_optional: &str,
    output_format: &str,
    multiline: &str,
    pretty_body: &str,
    include_comments: &str,
    max_depth: &str,
) -> Result<String, JsValue> {
    let max_depth = match max_depth.trim() {
        "" => 4,
        s => s.parse::<u32>().map_err(|_| {
            JsValue::from_str(&format!(
                "max_depth must be a whole number between 1 and 8 (got '{s}')"
            ))
        })?,
    };
    gizza_ai_openapi_to_curl_core::generate(
        spec,
        input_format,
        base_url,
        auth,
        auth_value,
        methods,
        tags,
        path_filter,
        truthy_or_default(include_optional, false),
        output_format,
        truthy_or_default(multiline, true),
        truthy_or_default(pretty_body, false),
        truthy_or_default(include_comments, true),
        max_depth,
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
