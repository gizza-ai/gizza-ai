//! Browser-facing wasm-bindgen wrapper for /tools/cmac-generate/.
//! Compiled with wasm-pack for the standalone /tools/cmac-generate/ page.
use wasm_bindgen::prelude::*;

/// Compute the AES-CMAC of `message` keyed by `key`.
///
/// The standalone tool page passes every field value as a string:
/// - `message`: the data to authenticate.
/// - `key`: the secret AES key (length picks AES-128/192/256: 16/24/32 bytes).
/// - `message_encoding` / `key_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
/// - `output_format`: `"hex"` (blank → hex) / `"base64"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
///
/// Throws a JS error string on an invalid encoding/format, an undecodable input,
/// or a key whose length is not 16/24/32 bytes.
#[wasm_bindgen]
pub fn run(
    message: &str,
    key: &str,
    message_encoding: &str,
    key_encoding: &str,
    output_format: &str,
    uppercase: &str,
) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_cmac_generate_core::cmac(
        message,
        key,
        message_encoding,
        key_encoding,
        output_format,
        uppercase,
    )
    .map_err(|e| JsValue::from_str(&e))
}
