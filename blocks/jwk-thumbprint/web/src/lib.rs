//! Browser-facing wasm-bindgen wrapper for /tools/jwk-thumbprint/.
use gizza_ai_jwk_thumbprint_core::thumbprint;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(jwk: &str) -> Result<String, JsValue> {
    let t = thumbprint(jwk).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "thumbprint (kid): {}\nkty: {}\ncanonical JSON: {}",
        t.thumbprint, t.kty, t.canonical
    ))
}
