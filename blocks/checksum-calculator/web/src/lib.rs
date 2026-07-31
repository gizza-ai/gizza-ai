//! Browser-facing wasm-bindgen wrapper for /tools/checksum-calculator/.
//! Compiled with wasm-pack for the standalone /tools/checksum-calculator/ page.
use wasm_bindgen::prelude::*;

/// Compute a CRC-family checksum of `text` and optionally verify it.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the input to checksum.
/// - `algorithm`: `"crc32"` (blank → crc32) / `"crc32c"` / `"crc16"` / `"crc8"`.
/// - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
/// - `output_format`: `"hex"` (blank → hex) / `"decimal"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
/// - `expected`: optional expected checksum; blank → no verification.
///
/// Throws a JS error string on an invalid algorithm/encoding/format or
/// undecodable input.
#[wasm_bindgen]
pub fn run(
    text: &str,
    algorithm: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: &str,
    expected: &str,
) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_checksum_calculator_core::checksum(
        text,
        algorithm,
        input_encoding,
        output_format,
        uppercase,
        expected,
    )
    .map_err(|e| JsValue::from_str(&e))
}
