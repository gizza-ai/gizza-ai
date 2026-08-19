//! Browser-facing wasm-bindgen wrapper for /tools/flask-session-sign/.
//! Field order MUST match page/meta.toml.
use gizza_ai_flask_session_sign_core::{
    sign_to_json, CompressMode, DigestMethod, KeyDerivation, SecretEncoding, SignOptions,
};
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_i64_field(name: &str, s: &str, default: i64) -> Result<i64, JsValue> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    trimmed
        .parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be an integer Unix timestamp")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    payload: &str,
    secret: &str,
    salt: &str,
    secret_encoding: &str,
    digest: &str,
    key_derivation: &str,
    timestamp: &str,
    legacy_epoch: &str,
    compress: &str,
    cookie_name: &str,
) -> Result<String, JsValue> {
    let opts = SignOptions {
        secret: secret.to_string(),
        salt: if salt.trim().is_empty() {
            "cookie-session".to_string()
        } else {
            salt.to_string()
        },
        secret_encoding: SecretEncoding::parse(secret_encoding)
            .map_err(|e| JsValue::from_str(&e))?,
        digest: DigestMethod::parse(digest).map_err(|e| JsValue::from_str(&e))?,
        key_derivation: KeyDerivation::parse(key_derivation).map_err(|e| JsValue::from_str(&e))?,
        timestamp: parse_i64_field("timestamp", timestamp, 0)?,
        legacy_epoch: truthy(legacy_epoch, false),
        compress: CompressMode::parse(compress).map_err(|e| JsValue::from_str(&e))?,
        cookie_name: if cookie_name.trim().is_empty() {
            "session".to_string()
        } else {
            cookie_name.to_string()
        },
        ..Default::default()
    };
    sign_to_json(payload, &opts).map_err(|e| JsValue::from_str(&e))
}
