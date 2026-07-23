//! Browser-facing wasm-bindgen wrapper for /tools/har-redact/.
//! Field order MUST match meta.toml: har, cookies, auth_headers,
//! extra_headers, query_params, sensitive_params, bodies, placeholder,
//! output, pretty. Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_har_redact_core::redact_har;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// A checkbox that defaults ON: an empty field (deep-link without the param)
/// keeps the default; an explicit value decides.
fn checked_default_on(v: &str) -> bool {
    let t = v.trim();
    if t.is_empty() {
        true
    } else {
        truthy(t)
    }
}

#[wasm_bindgen]
pub fn run(
    har: &str,
    cookies: &str,
    auth_headers: &str,
    extra_headers: &str,
    query_params: &str,
    sensitive_params: &str,
    bodies: &str,
    placeholder: &str,
    output: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    // Empty selects/fields (deep-link without the param) fall back to the
    // descriptor defaults; the core still validates the enum values.
    let bodies = if bodies.trim().is_empty() { "response" } else { bodies.trim() };
    let output = if output.trim().is_empty() { "har" } else { output.trim() };
    let placeholder = if placeholder.is_empty() { "[REDACTED]" } else { placeholder };
    redact_har(
        har,
        checked_default_on(cookies),
        checked_default_on(auth_headers),
        extra_headers,
        checked_default_on(query_params),
        sensitive_params,
        bodies,
        placeholder,
        output,
        truthy(pretty),
    )
    .map_err(|e| JsValue::from_str(&e))
}
