//! Browser-facing wasm-bindgen wrapper for /tools/x25519-ecdh/.
//! Field order MUST match page/meta.toml: private_key, peer_public_key, kdf,
//! kdf_salt, kdf_info, kdf_length, encoding, include_pem. Every control arrives
//! as a string — checkboxes as "true"/"false", the number field as text — and a
//! blank value (an omitted `?param=`) falls back to the descriptor default.
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    matches!(t.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn int_value(s: &str, default: usize) -> usize {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    t.parse::<usize>().unwrap_or(default)
}

/// A blank select/query-param means "unset", which must resolve to the page
/// default — not to whatever the core parser maps an empty string to.
fn or_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() {
        default
    } else {
        s
    }
}

#[wasm_bindgen]
pub fn run(
    private_key: &str,
    peer_public_key: &str,
    kdf: &str,
    kdf_salt: &str,
    kdf_info: &str,
    kdf_length: &str,
    encoding: &str,
    include_pem: &str,
) -> Result<String, JsValue> {
    gizza_ai_x25519_ecdh_core::run(
        private_key,
        peer_public_key,
        or_default(kdf, "hkdf-sha256"),
        kdf_salt,
        kdf_info,
        int_value(kdf_length, 32),
        or_default(encoding, "hex"),
        truthy(include_pem, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
