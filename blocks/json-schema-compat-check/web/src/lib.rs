//! Browser-facing wasm-bindgen wrapper for /tools/json-schema-compat-check/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    old_schema: &str,
    new_schema: &str,
    direction: &str,
    strict_required: &str,
) -> Result<String, JsValue> {
    if old_schema.trim().is_empty() || new_schema.trim().is_empty() {
        return Ok("Paste both the old and new JSON Schema documents to compare.".to_string());
    }
    let strict_required = matches!(strict_required.trim(), "true" | "1" | "yes" | "on");
    gizza_ai_json_schema_compat_check_core::run(
        old_schema,
        new_schema,
        if direction.trim().is_empty() {
            "both"
        } else {
            direction
        },
        strict_required,
    )
    .map_err(|e| JsValue::from_str(&e))
}
