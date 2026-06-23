//! Browser-facing wasm-bindgen wrapper for /tools/hkdf-derive/.
//! Field order MUST match meta.toml: ikm, mode, ikm_encoding, salt,
//! salt_encoding, info, info_encoding, hash, length, encoding.
use gizza_ai_hkdf_derive_core::{derive, extract};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    ikm: &str,
    mode: &str,
    ikm_encoding: &str,
    salt: &str,
    salt_encoding: &str,
    info: &str,
    info_encoding: &str,
    hash: &str,
    length: &str,
    encoding: &str,
) -> Result<String, JsValue> {
    let len: usize = length.trim().parse().unwrap_or(32);
    let ikm_enc = if ikm_encoding.trim().is_empty() { "utf8" } else { ikm_encoding };
    let salt_enc = if salt_encoding.trim().is_empty() { "utf8" } else { salt_encoding };
    let info_enc = if info_encoding.trim().is_empty() { "utf8" } else { info_encoding };
    let h = if hash.trim().is_empty() { "sha256" } else { hash };
    let enc = if encoding.trim().is_empty() { "hex" } else { encoding };
    match mode.trim().to_ascii_lowercase().as_str() {
        "extract" => {
            extract(ikm, ikm_enc, salt, salt_enc, h, enc).map_err(|e| JsValue::from_str(&e))
        }
        _ => derive(ikm, ikm_enc, salt, salt_enc, info, info_enc, h, len, enc)
            .map_err(|e| JsValue::from_str(&e)),
    }
}
