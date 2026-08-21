//! Browser-facing wasm-bindgen wrapper for /tools/vcard-validator/.
//! Compiled with wasm-pack for the standalone /tools/vcard-validator/ page.
use gizza_ai_vcard_validator_core::{validate, Output, Version};
use wasm_bindgen::prelude::*;

/// Validate the vCard text in `data`.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `version`: `"auto"`/`"2.1"`/`"3.0"`/`"4.0"` (blank → auto).
/// - `default_country`: ISO-3166 alpha-2 hint for national-format TEL values.
/// - `check_email` / `check_phone`: `"true"`/`"1"`/`"yes"`/`"on"` → on (blank →
///   on, the descriptor default); anything else → off.
/// - `required_properties`: comma-separated property names every card must have.
/// - `output`: `"report"`/`"json"` (blank → report).
///
/// Throws a JS error string on an invalid `version`, `output` or
/// `default_country`, and when the input contains no vCard at all.
#[wasm_bindgen]
pub fn run(
    data: &str,
    version: &str,
    default_country: &str,
    check_email: &str,
    check_phone: &str,
    required_properties: &str,
    output: &str,
) -> Result<String, JsValue> {
    let version = Version::parse(version).map_err(|e| JsValue::from_str(&e))?;
    let output = Output::parse(output).map_err(|e| JsValue::from_str(&e))?;
    validate(
        data,
        version,
        default_country,
        truthy(check_email),
        truthy(check_phone),
        required_properties,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Positive-truthy parse: a blank field defaults to ON (both boolean params
/// default to `true` in the descriptor, so a checked box → `"true"`, an
/// unchecked box → `"false"`, and a URL-prefill omitting the field → blank).
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "yes" | "on"
    )
}
