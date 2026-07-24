//! Browser-facing wasm-bindgen wrapper for /tools/hash-ioc-match/.
//! Compiled with wasm-pack for the standalone /tools/hash-ioc-match/ page.
use wasm_bindgen::prelude::*;

/// Hash `input` and flag it against a `blocklist` of known-bad hashes.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the file content to hash (interpreted per `input_encoding`).
/// - `blocklist`: pasted known-bad hashes, any format.
/// - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
///
/// Returns a multi-line report (FLAGGED/CLEAN + matched digests + every computed
/// hash). Throws a JS error string on an invalid encoding or an undecodable
/// hex/base64 input.
#[wasm_bindgen]
pub fn run(input: &str, blocklist: &str, input_encoding: &str) -> Result<String, JsValue> {
    gizza_ai_hash_ioc_match_core::report(input, blocklist, input_encoding)
        .map_err(|e| JsValue::from_str(&e))
}
