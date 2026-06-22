//! Browser-facing wasm-bindgen wrapper for /tools/fernet-token/.
//! Field order MUST match meta.toml: text, key, mode, ttl.
//! The page target (wasm32-unknown-unknown) has no std clock, so the timestamp
//! comes from js_sys::Date; randomness comes from getrandom's js backend.
use gizza_ai_fernet_token_core::{decrypt, encrypt, inspect, key_from_bytes};
use wasm_bindgen::prelude::*;

fn now_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|e| format!("randomness unavailable: {e}"))?;
    Ok(b)
}

fn iso8601_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn run_inner(text: &str, key: &str, mode: &str, ttl: &str) -> Result<String, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "encrypt" | "" => {
            let key = if key.trim().is_empty() {
                key_from_bytes(&random_bytes::<32>()?)
            } else {
                key.trim().to_string()
            };
            let iv = random_bytes::<16>()?;
            let token = encrypt(&key, text.as_bytes(), now_secs(), &iv)?;
            Ok(format!("{token}\n\nKey (save this to decrypt): {key}"))
        }
        "decrypt" => {
            if key.trim().is_empty() {
                return Err("decrypt requires the Fernet key the token was created with".into());
            }
            let ttl: u64 = ttl.trim().parse().unwrap_or(0);
            let ttl = if ttl == 0 { None } else { Some(ttl) };
            let d = decrypt(key.trim(), text.trim(), now_secs(), ttl)?;
            let plaintext = String::from_utf8(d.plaintext)
                .map_err(|_| "decrypted bytes are not valid UTF-8 text".to_string())?;
            Ok(format!("{plaintext}\n\nCreated: {}", iso8601_utc(d.timestamp)))
        }
        "inspect" => {
            let k = if key.trim().is_empty() { None } else { Some(key.trim()) };
            let i = inspect(text.trim(), k)?;
            let hmac = match i.hmac_valid {
                Some(true) => "\nHMAC: valid (key matches)".to_string(),
                Some(false) => "\nHMAC: INVALID (wrong key or tampered)".to_string(),
                None => String::new(),
            };
            Ok(format!(
                "Version: 0x{:02x}\nCreated: {}\nIV: {}\nCiphertext: {} bytes\nHMAC: {}{}",
                i.version,
                iso8601_utc(i.timestamp),
                i.iv_hex,
                i.ciphertext_len,
                i.hmac_hex,
                hmac
            ))
        }
        other => Err(format!("unknown mode '{other}' (use 'encrypt', 'decrypt', or 'inspect')")),
    }
}

#[wasm_bindgen]
pub fn run(text: &str, key: &str, mode: &str, ttl: &str) -> Result<String, JsValue> {
    run_inner(text, key, mode, ttl).map_err(|e| JsValue::from_str(&e))
}
