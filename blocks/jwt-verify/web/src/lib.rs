//! Browser-facing wasm-bindgen wrapper for /tools/jwt-verify/.
//! Field order MUST match meta.toml: token, key, algorithm, issuer, audience, leeway.
//! The current time comes from the browser (`Date.now()`), since
//! wasm32-unknown-unknown has no std clock.
use gizza_ai_jwt_verify_core::{verify, Alg, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    token: &str,
    key: &str,
    algorithm: &str,
    issuer: &str,
    audience: &str,
    leeway: &str,
) -> Result<String, JsValue> {
    let expected_alg = if algorithm.trim().is_empty() {
        None
    } else {
        Some(Alg::parse(algorithm).map_err(|e| JsValue::from_str(&e))?)
    };
    let leeway: i64 = leeway.trim().parse().unwrap_or(0);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let opts = Options {
        expected_alg,
        expected_iss: non_empty(issuer),
        expected_aud: non_empty(audience),
        now,
        leeway,
    };
    let res = verify(token, key, &opts).map_err(|e| JsValue::from_str(&e))?;
    // Pretty-print the structured result for the page output.
    serde_json::to_string_pretty(&res.to_json()).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
