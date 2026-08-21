//! Browser-facing wasm-bindgen wrapper for /tools/openapi-stub-from-json/.
//! Field order MUST match meta.toml: request_json, response_json, method, path,
//! query, status, content_type, operation_id, tag, title, api_version,
//! server_url, security, format, components, extract_nested, required_props,
//! detect_formats, include_examples, include_error_responses.
use gizza_ai_openapi_stub_from_json_core::{generate, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    request_json: &str,
    response_json: &str,
    method: &str,
    path: &str,
    query: &str,
    status: &str,
    content_type: &str,
    operation_id: &str,
    tag: &str,
    title: &str,
    api_version: &str,
    server_url: &str,
    security: &str,
    format: &str,
    components: &str,
    extract_nested: &str,
    required_props: &str,
    detect_formats: &str,
    include_examples: &str,
    include_error_responses: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let status = match status.trim() {
        "" => d.status,
        s => s.parse::<u16>().map_err(|_| {
            JsValue::from_str(&format!("status must be a whole number (got '{s}')"))
        })?,
    };
    let opts = Options {
        method: pick(method, &d.method),
        path: pick(path, &d.path),
        operation_id: operation_id.to_string(),
        tag: tag.to_string(),
        status,
        content_type: pick(content_type, &d.content_type),
        title: title.to_string(),
        api_version: api_version.to_string(),
        server_url: server_url.to_string(),
        query: query.to_string(),
        security: pick(security, &d.security),
        format: pick(format, &d.format),
        components: truthy_or_default(components, d.components),
        extract_nested: truthy_or_default(extract_nested, d.extract_nested),
        required_props: truthy_or_default(required_props, d.required_props),
        detect_formats: truthy_or_default(detect_formats, d.detect_formats),
        include_examples: truthy_or_default(include_examples, d.include_examples),
        include_error_responses: truthy_or_default(
            include_error_responses,
            d.include_error_responses,
        ),
    };
    generate(request_json, response_json, &opts).map_err(|e| JsValue::from_str(&e))
}

/// A blank field means "use the documented default".
fn pick(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

/// Checkboxes arrive as "true"/"false"; blank means the descriptor default.
fn truthy_or_default(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}
