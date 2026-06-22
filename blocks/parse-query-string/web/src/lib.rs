//! Browser-facing wasm-bindgen wrapper for /tools/parse-query-string/.
//! Compiled with wasm-pack for the standalone page. The page passes every
//! field value as a string; `plus_as_space` is a checkbox (default checked →
//! "true"). The page shows the human-readable `render` view (ordered pairs +
//! the structured JSON).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(query: &str, plus_as_space: &str) -> Result<String, JsValue> {
    let plus = matches!(
        plus_as_space.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_parse_query_string_core::render(query, plus).map_err(|e| JsValue::from_str(&e))
}
