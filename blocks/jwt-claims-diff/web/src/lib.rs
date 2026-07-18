//! Browser-facing wasm-bindgen wrapper for /tools/jwt-claims-diff/.
//! Field order MUST match meta.toml: left, right, include_header, indent.
use gizza_ai_jwt_claims_diff_core::diff_jwts;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(left: &str, right: &str, include_header: &str, indent: &str) -> Result<String, JsValue> {
    // A default-true checkbox arrives as "true"/"false"; treat any positive
    // truthy string as checked.
    let include = matches!(
        include_header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let n: usize = indent.trim().parse().unwrap_or(2);
    diff_jwts(left, right, include, n).map_err(|e| JsValue::from_str(&e))
}
