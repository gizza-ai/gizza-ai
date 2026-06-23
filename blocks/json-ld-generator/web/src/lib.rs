//! Browser-facing wasm-bindgen wrapper for /tools/json-ld-generator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Generate schema.org JSON-LD markup.
///
/// The standalone tool page passes every field value as a string:
/// - `schema_type`: a schema.org @type (blank → Article).
/// - `fields`: newline-separated `name = value` property pairs.
/// - `faq`: newline-separated `Question? | Answer` pairs (FAQPage only).
/// - `wrap_script`: `"true"`/`"1"`/`"yes"`/`"on"` → wrap output in a
///   `<script type="application/ld+json">` tag; anything else → raw JSON.
///
/// Throws a JS error string on an unknown type or an unparseable line.
#[wasm_bindgen]
pub fn run(
    schema_type: &str,
    fields: &str,
    faq: &str,
    wrap_script: &str,
) -> Result<String, JsValue> {
    let wrap = matches!(
        wrap_script.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_json_ld_generator_core::generate(schema_type, fields, faq, wrap)
        .map_err(|e| JsValue::from_str(&e))
}
