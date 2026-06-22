//! Browser-facing wasm-bindgen wrapper for /tools/classical-cipher-tool/.
//! Compiled with wasm-pack for the standalone page. Every field value arrives as
//! a string; the core parses the key per cipher.
use wasm_bindgen::prelude::*;

/// Encrypt / decrypt `text` with a classical cipher.
///
/// - `cipher`: `"caesar"` (default) | `"vigenere"` | `"atbash"` | `"rail-fence"`.
/// - `operation`: `"encrypt"` (default) | `"decrypt"` | `"brute-force"`.
/// - `key`: cipher-specific (Caesar shift, Vigenère keyword, rail count).
///
/// Throws a JS error string on an invalid cipher/operation or key.
#[wasm_bindgen]
pub fn run(cipher: &str, operation: &str, key: &str, text: &str) -> Result<String, JsValue> {
    gizza_ai_classical_cipher_tool_core::run(cipher, operation, key, text)
        .map_err(|e| JsValue::from_str(&e))
}
