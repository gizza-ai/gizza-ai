//! Browser-facing wasm-bindgen wrapper for /tools/har-body-stripper/.
//! Field order MUST match meta.toml: har, strip, only_mime, min_bytes,
//! output, pretty. Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_har_body_stripper_core::strip_bodies;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    har: &str,
    strip: &str,
    only_mime: &str,
    min_bytes: &str,
    output: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    // Empty selects/fields (deep-link without the param) fall back to the
    // descriptor defaults; the core still validates the enum values.
    let strip = if strip.trim().is_empty() { "both" } else { strip.trim() };
    let output = if output.trim().is_empty() { "har" } else { output.trim() };
    let min_bytes = {
        let t = min_bytes.trim();
        if t.is_empty() {
            0
        } else {
            t.parse::<u64>()
                .map_err(|_| JsValue::from_str("min_bytes must be a whole number of bytes (0 or more)"))?
        }
    };
    strip_bodies(har, strip, only_mime, min_bytes, output, truthy(pretty))
        .map_err(|e| JsValue::from_str(&e))
}
