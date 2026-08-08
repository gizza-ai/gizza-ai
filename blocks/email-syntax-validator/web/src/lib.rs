//! Browser-facing wasm-bindgen wrapper for /tools/email-syntax-validator/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim();
    if t.is_empty() { return default; }
    matches!(t.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}
fn or_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() { default } else { s }
}

#[wasm_bindgen]
pub fn run(email: &str, check_disposable: &str, check_role: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_email_syntax_validator_core::run(
        email,
        truthy(check_disposable, true),
        truthy(check_role, true),
        or_default(format, "report"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
