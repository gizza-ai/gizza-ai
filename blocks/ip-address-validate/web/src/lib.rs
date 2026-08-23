//! Browser-facing wasm-bindgen wrapper for /tools/ip-address-validate/.
//! Argument order MUST match the `[[input]]` order in page/meta.toml:
//! addresses, family, output, ipv6_form, allow_prefix, allow_port,
//! allow_leading_zeros, dedupe. Every field arrives as a string (checkboxes
//! send "true"/"false"); empty selects fall back to the schema defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// A checkbox that is absent from a deep link should keep its schema default
/// rather than silently reading as false.
fn flag(s: &str, default: bool) -> bool {
    if s.trim().is_empty() {
        default
    } else {
        truthy(s)
    }
}

fn or_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() {
        default
    } else {
        s
    }
}

#[wasm_bindgen]
pub fn run(
    addresses: &str,
    family: &str,
    output: &str,
    ipv6_form: &str,
    allow_prefix: &str,
    allow_port: &str,
    allow_leading_zeros: &str,
    dedupe: &str,
) -> Result<String, JsValue> {
    gizza_ai_ip_address_validate_core::run(
        addresses,
        or_default(family, "any"),
        flag(allow_prefix, true),
        flag(allow_port, true),
        flag(allow_leading_zeros, false),
        or_default(ipv6_form, "compressed"),
        flag(dedupe, false),
        or_default(output, "report"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
