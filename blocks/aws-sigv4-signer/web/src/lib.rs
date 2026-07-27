//! Browser-facing wasm-bindgen wrapper for /tools/aws-sigv4-signer/.
//! Compiled with wasm-pack for the standalone /tools/aws-sigv4-signer/ page.
//!
//! The page passes every field value as a string, in the meta.toml `[[input]]`
//! order. Booleans arrive as "true"/"false"; blank `amz_date` is filled with the
//! current UTC time from the browser clock (the page target has no std clock).
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Compute an AWS Signature Version 4 signature for the given request.
///
/// Returns the requested artifact (`output`), throwing a JS error string on any
/// invalid input (bad URL, timestamp, header line, method, or output selector).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    url: &str,
    region: &str,
    service: &str,
    access_key: &str,
    secret_key: &str,
    method: &str,
    session_token: &str,
    payload: &str,
    headers: &str,
    amz_date: &str,
    unsigned_payload: &str,
    sign_content_sha256: &str,
    output: &str,
) -> Result<String, JsValue> {
    let amz_date = if amz_date.trim().is_empty() {
        let epoch = (js_sys::Date::now() / 1000.0) as i64;
        gizza_ai_aws_sigv4_signer_core::format_amz_date(epoch)
    } else {
        amz_date.to_string()
    };
    gizza_ai_aws_sigv4_signer_core::sign(
        method,
        url,
        region,
        service,
        access_key,
        secret_key,
        session_token,
        payload,
        headers,
        &amz_date,
        truthy(unsigned_payload),
        truthy(sign_content_sha256),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
