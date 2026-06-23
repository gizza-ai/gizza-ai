//! Browser-facing wasm-bindgen wrapper for /tools/evp-derive/.
//! Field order MUST match meta.toml: password, salt, salt_encoding, hash,
//! key_length, iv_length, count, encoding.
use gizza_ai_evp_derive_core::derive;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    password: &str,
    salt: &str,
    salt_encoding: &str,
    hash: &str,
    key_length: &str,
    iv_length: &str,
    count: &str,
    encoding: &str,
) -> Result<String, JsValue> {
    let key_len: usize = key_length.trim().parse().unwrap_or(32);
    let iv_len: usize = iv_length.trim().parse().unwrap_or(16);
    let cnt: u32 = count.trim().parse().unwrap_or(1);
    let salt_enc = if salt_encoding.trim().is_empty() { "utf8" } else { salt_encoding };
    let h = if hash.trim().is_empty() { "md5" } else { hash };
    let enc = if encoding.trim().is_empty() { "hex" } else { encoding };
    let (key, iv) = derive(password, salt, salt_enc, h, key_len, iv_len, cnt, enc)
        .map_err(|e| JsValue::from_str(&e))?;
    let mut out = format!("key: {key}");
    if !iv.is_empty() {
        out.push_str(&format!("\niv:  {iv}"));
    }
    Ok(out)
}
