//! Browser-facing wasm-bindgen wrapper for /tools/webhook-signature-generator/.
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    payload: &str,
    secret: &str,
    provider: &str,
    timestamp: &str,
    message_id: &str,
    url: &str,
    algorithm: &str,
    encoding: &str,
    secret_encoding: &str,
    template: &str,
    header_name: &str,
    signature_prefix: &str,
    output: &str,
) -> Result<String, JsValue> {
    let timestamp = if timestamp.trim().is_empty() {
        gizza_ai_webhook_signature_generator_core::format_timestamp((js_sys::Date::now() / 1000.0) as i64)
    } else {
        timestamp.to_string()
    };
    gizza_ai_webhook_signature_generator_core::run(
        payload,
        secret,
        provider,
        &timestamp,
        message_id,
        url,
        algorithm,
        encoding,
        secret_encoding,
        template,
        header_name,
        signature_prefix,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
