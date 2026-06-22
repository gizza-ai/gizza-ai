//! Browser-facing wasm-bindgen wrapper for /tools/ciphersaber2/.
//! Field order MUST match meta.toml: data, operation, key, key_format, rounds, iv, format.
use gizza_ai_ciphersaber2_core::{decrypt, encrypt, Encoding, KeyFormat, IV_LEN};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    key: &str,
    key_format: &str,
    rounds: &str,
    iv: &str,
    format: &str,
) -> Result<String, JsValue> {
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let rounds_n = if rounds.trim().is_empty() {
        20
    } else {
        rounds
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("rounds must be a positive whole number"))?
    };
    if rounds_n < 1 {
        return Err(JsValue::from_str("rounds must be >= 1"));
    }
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, kf, rounds_n, fmt),
        _ => {
            let iv_opt = {
                let s = iv.trim();
                if s.is_empty() {
                    None
                } else {
                    let bytes = fmt.decode(s).map_err(|e| JsValue::from_str(&e))?;
                    if bytes.len() != IV_LEN {
                        return Err(JsValue::from_str(&format!(
                            "iv must be exactly {IV_LEN} bytes"
                        )));
                    }
                    let mut a = [0u8; IV_LEN];
                    a.copy_from_slice(&bytes);
                    Some(a)
                }
            };
            encrypt(data, key, kf, rounds_n, iv_opt, fmt)
        }
    };
    r.map_err(|e| JsValue::from_str(&e))
}
