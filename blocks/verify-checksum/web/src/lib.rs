//! Browser-facing wasm-bindgen wrapper for /tools/verify-checksum/.
//! Compiled with wasm-pack for the standalone /tools/verify-checksum/ page.
use wasm_bindgen::prelude::*;

/// Verify that `text` matches the `expected` checksum.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the input data to hash and check.
/// - `expected`: the expected checksum (hex, optionally `0x`-prefixed and any
///   case, or standard base64).
/// - `algorithm`: `"auto"` (blank → auto) or an explicit id (`"sha256"`, …).
/// - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
///
/// Returns a multi-line report (MATCH/MISMATCH + algorithm + expected/actual
/// digests). Throws a JS error string on an invalid algorithm/encoding or an
/// undecodable input/expected value.
#[wasm_bindgen]
pub fn run(
    text: &str,
    expected: &str,
    algorithm: &str,
    input_encoding: &str,
) -> Result<String, JsValue> {
    gizza_ai_verify_checksum_core::verify_report(text, expected, algorithm, input_encoding)
        .map_err(|e| JsValue::from_str(&e))
}
