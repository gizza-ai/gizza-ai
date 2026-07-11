//! Browser-facing wasm-bindgen wrapper for /tools/ip-log-anonymizer/.
//! The page driver passes every field as a raw string, so we parse the numeric
//! and boolean fields here (blank → the core's default) and funnel everything
//! through the shared `anonymize` validation.
use gizza_ai_ip_log_anonymizer_core::anonymize;
use wasm_bindgen::prelude::*;

fn parse_u32(s: &str, default: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse::<u32>().unwrap_or(default)
    }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    ipv4_octets: &str,
    ipv6_groups: &str,
    salt: &str,
    hash_length: &str,
    replacement: &str,
    skip_private: &str,
) -> Result<String, JsValue> {
    let mode = if mode.trim().is_empty() { "mask" } else { mode };
    let replacement = if replacement.is_empty() { "[IP]" } else { replacement };
    anonymize(
        text,
        mode,
        parse_u32(ipv4_octets, 1),
        parse_u32(ipv6_groups, 5),
        salt,
        parse_u32(hash_length, 12),
        replacement,
        truthy(skip_private),
    )
    .map_err(|e| JsValue::from_str(&e))
}
