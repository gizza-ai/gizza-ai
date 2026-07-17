//! Browser-facing wasm-bindgen wrapper for /tools/vcard-normalize/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    default_country: &str,
    name_case: &str,
    lowercase_email: &str,
) -> Result<String, JsValue> {
    // lowercase_email is a default-ON checkbox: blank means the schema default is
    // still true unless the page sends an explicit false/off value.
    let lowercase_email = !matches!(
        lowercase_email.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    let case = gizza_ai_vcard_normalize_core::NameCase::parse(name_case)
        .map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_vcard_normalize_core::run(data, default_country, case, lowercase_email)
        .map_err(|e| JsValue::from_str(&e))
}