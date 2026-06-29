//! Browser-facing wasm-bindgen wrapper for /tools/nt-hash/.
//! Compiled with wasm-pack for the standalone /tools/nt-hash/ page.
use wasm_bindgen::prelude::*;

/// Compute the NT (NTLM) hash of `password`.
///
/// The standalone tool page passes every field value as a string:
/// - `password`: the password/text to hash.
/// - `output_format`: `"hex"` (blank → hex) / `"base64"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
///
/// Throws a JS error string on an invalid output format.
#[wasm_bindgen]
pub fn run(password: &str, output_format: &str, uppercase: &str) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_nt_hash_core::hash(password, output_format, uppercase).map_err(|e| JsValue::from_str(&e))
}
