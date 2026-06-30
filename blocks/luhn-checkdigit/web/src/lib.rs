//! Browser-facing wasm-bindgen wrapper for /tools/luhn-checkdigit/.
//! Single field: number (the payload). Returns a human-readable summary.
use gizza_ai_luhn_checkdigit_core::check_digit;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(number: &str) -> Result<String, JsValue> {
    let r = check_digit(number).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "Check digit: {}\nFull number: {}\n({} digits, payload {})",
        r.check_digit, r.full_number, r.length, r.payload
    ))
}
