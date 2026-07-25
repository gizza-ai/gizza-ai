//! Browser-facing wasm-bindgen wrapper for /tools/ics-parse/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string; the boolean checkboxes arrive as "true"/"false" and blank
//! selects fall back to the descriptor default inside the core.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    ics: &str,
    date_format: &str,
    pretty: &str,
    include_description: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        )
    };
    gizza_ai_ics_parse_core::parse_ics_str(
        ics,
        date_format,
        truthy(pretty),
        truthy(include_description),
    )
    .map_err(|e| JsValue::from_str(&e))
}
