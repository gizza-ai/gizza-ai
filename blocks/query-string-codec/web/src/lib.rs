//! Browser-facing wasm-bindgen wrapper for /tools/query-string-codec/.
//! The page passes every field value as a string; the booleans are checkboxes
//! (parsed positive-truthy here), `direction`/`array_style` are `<select>`s.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

#[wasm_bindgen]
pub fn run(
    direction: &str,
    input: &str,
    array_style: &str,
    space_as_plus: &str,
    sort_keys: &str,
    prefix_question_mark: &str,
) -> Result<String, JsValue> {
    gizza_ai_query_string_codec_core::run(
        direction,
        input,
        array_style,
        truthy(space_as_plus),
        truthy(sort_keys),
        truthy(prefix_question_mark),
    )
    .map_err(|e| JsValue::from_str(&e))
}
