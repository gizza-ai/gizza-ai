//! Browser-facing wasm-bindgen wrapper for /tools/jwt-decode/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(token: &str, leeway: &str) -> Result<String, JsValue> {
    let leeway: i64 = leeway.trim().parse().unwrap_or(0);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    
    let res = gizza_ai_jwt_decode_core::decode_jwt(token, now, leeway)
        .map_err(|e| JsValue::from_str(&e))?;
    
    serde_json::to_string_pretty(&res.to_json())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
