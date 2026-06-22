//! Browser-facing wasm-bindgen wrapper for /tools/keccak-hash/.
//! Compiled with wasm-pack for the standalone /tools/keccak-hash/ page.
use wasm_bindgen::prelude::*;

/// Compute the Keccak digest of `text` with the selected variant.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the input to hash.
/// - `algorithm`: `"keccak-256"` (blank → keccak-256) / `"keccak-512"`.
/// - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
/// - `output_format`: `"hex"` (blank → hex) / `"base64"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
///
/// Throws a JS error string on an invalid variant/encoding/format or
/// undecodable input.
#[wasm_bindgen]
pub fn run(
    text: &str,
    algorithm: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: &str,
) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_keccak_hash_core::hash(text, algorithm, input_encoding, output_format, uppercase)
        .map_err(|e| JsValue::from_str(&e))
}
