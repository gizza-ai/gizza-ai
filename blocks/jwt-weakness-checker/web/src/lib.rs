//! Browser-facing wasm-bindgen wrapper for /tools/jwt-weakness-checker/.
//! Argument order MUST match the `[[input]]` order in page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    token: &str,
    wordlist: &str,
    max_exp_days: &str,
    leeway: &str,
) -> Result<String, JsValue> {
    let max_exp_days: f64 = max_exp_days.trim().parse().unwrap_or(30.0);
    let leeway: i64 = leeway.trim().parse().unwrap_or(0);
    // wasm32-unknown-unknown has no std clock — take the time from JS.
    let now = (js_sys::Date::now() / 1000.0) as i64;

    let res = gizza_ai_jwt_weakness_checker_core::audit(token, now, leeway, max_exp_days, wordlist)
        .map_err(|e| JsValue::from_str(&e))?;

    serde_json::to_string_pretty(&res.to_json()).map_err(|e| JsValue::from_str(&e.to_string()))
}
