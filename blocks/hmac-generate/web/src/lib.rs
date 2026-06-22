//! Browser-facing wasm-bindgen wrapper for /tools/hmac-generate/.
//! Compiled with wasm-pack for the standalone /tools/hmac-generate/ page.
use wasm_bindgen::prelude::*;

/// Compute the HMAC of `message` keyed by `key` with the selected algorithm.
///
/// The standalone tool page passes every field value as a string:
/// - `message`: the data to authenticate.
/// - `key`: the secret key.
/// - `algorithm`: e.g. `"sha256"` (blank → sha256), `"sha1"`, `"sha3-512"`, …
/// - `message_encoding` / `key_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
/// - `output_format`: `"hex"` (blank → hex) / `"base64"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
///
/// Throws a JS error string on an invalid algorithm/encoding/format or
/// undecodable input.
#[wasm_bindgen]
pub fn run(
    message: &str,
    key: &str,
    algorithm: &str,
    message_encoding: &str,
    key_encoding: &str,
    output_format: &str,
    uppercase: &str,
) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_hmac_generate_core::hmac(
        message,
        key,
        algorithm,
        message_encoding,
        key_encoding,
        output_format,
        uppercase,
    )
    .map_err(|e| JsValue::from_str(&e))
}
