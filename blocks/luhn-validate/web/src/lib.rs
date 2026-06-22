//! Browser-facing wasm-bindgen wrapper for /tools/luhn-validate/.
//! Single field: number. Returns a human-readable summary line.
use gizza_ai_luhn_validate_core::check;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(number: &str) -> Result<String, JsValue> {
    let r = check(number).map_err(|e| JsValue::from_str(&e))?;
    let mut out = if r.valid {
        format!("VALID — passes the Luhn check ({} digits)", r.length)
    } else {
        format!(
            "INVALID — fails the Luhn check ({} digits). Correct last digit would be {}.",
            r.length, r.expected_check_digit
        )
    };
    if let Some(b) = r.brand {
        out.push_str(&format!("\nDetected brand: {b}"));
    }
    Ok(out)
}
