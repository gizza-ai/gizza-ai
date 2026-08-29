//! Browser-facing wasm-bindgen wrapper for /tools/dkim-generate/.
//! Argument order MUST match `page/meta.toml`'s `[[input]]` order. Every field
//! arrives as a string (a checkbox marshals as "true"/"false"), so the boolean
//! is parsed here and the core owns all validation — one code path for chat,
//! CLI and page.
use wasm_bindgen::prelude::*;

/// The checkbox states the page runtime can send, plus the forms a hand-written
/// `?include_hash=` deep link is likely to use.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    domain: &str,
    selector: &str,
    key_type: &str,
    output: &str,
    include_hash: &str,
    flags: &str,
    existing_key: &str,
) -> Result<String, JsValue> {
    // Empty fields fall back to the descriptor defaults; an empty checkbox
    // string only happens when the field is missing, and the tag is on by
    // default, so absence must not silently drop h=sha256.
    let selector = if selector.trim().is_empty() {
        "mail"
    } else {
        selector
    };
    let key_type = if key_type.trim().is_empty() {
        "rsa-2048"
    } else {
        key_type
    };
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output
    };
    let flags = if flags.trim().is_empty() {
        "none"
    } else {
        flags
    };
    let include_hash = include_hash.trim().is_empty() || truthy(include_hash);

    gizza_ai_dkim_generate_core::run(
        domain,
        selector,
        key_type,
        output,
        include_hash,
        flags,
        existing_key,
    )
    .map_err(|e| JsValue::from_str(&e))
}
