//! Browser-facing wasm-bindgen wrapper for /tools/password-export-convert/.
//! Field order MUST match meta.toml: data, from, to, include_totp,
//! include_extra_fields, skip_empty_passwords, default_folder. Fields arrive as
//! strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    from: &str,
    to: &str,
    include_totp: &str,
    include_extra_fields: &str,
    skip_empty_passwords: &str,
    default_folder: &str,
) -> Result<String, JsValue> {
    gizza_ai_password_export_convert_core::run(
        data,
        or_default(from, "auto"),
        or_default(to, "keepass-csv"),
        truthy(include_totp),
        truthy(include_extra_fields),
        truthy(skip_empty_passwords),
        default_folder,
    )
    .map_err(|e| JsValue::from_str(&e))
}
