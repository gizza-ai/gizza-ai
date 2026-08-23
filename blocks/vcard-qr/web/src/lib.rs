//! Browser-facing wasm-bindgen wrapper for /tools/vcard-qr/.
use gizza_ai_vcard_qr_core::Options;
use wasm_bindgen::prelude::*;

/// The page sends checkboxes as "true"/"false"; anything positive-truthy counts.
fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    }
}

fn or<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    first_name: &str,
    last_name: &str,
    organization: &str,
    job_title: &str,
    mobile: &str,
    phone: &str,
    email: &str,
    website: &str,
    street: &str,
    city: &str,
    region: &str,
    postal_code: &str,
    country: &str,
    note: &str,
    birthday: &str,
    version: &str,
    error_correction: &str,
    size: &str,
    foreground: &str,
    background: &str,
    show_details: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        first_name,
        last_name,
        organization,
        job_title,
        mobile,
        phone,
        email,
        website,
        street,
        city,
        region,
        postal_code,
        country,
        note,
        birthday,
        version: or(version, "3.0"),
        error_correction: or(error_correction, "M"),
        size: size.trim().parse::<u32>().unwrap_or(512),
        foreground: or(foreground, "#000000"),
        background: or(background, "#ffffff"),
        show_details: flag(show_details, true),
    };
    gizza_ai_vcard_qr_core::run(&opts).map_err(|e| JsValue::from_str(&e))
}
