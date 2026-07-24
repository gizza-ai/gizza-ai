//! Browser-facing wasm-bindgen wrapper for /tools/json-schema-batch-validate/.
//! Compiled with wasm-pack for the standalone page. Fields arrive as strings in
//! `page/meta.toml` order: schema, records, input_format, draft, max_errors,
//! output. Blank enum fields fall back to their schema defaults.
use wasm_bindgen::prelude::*;

fn parse_max_errors(s: &str) -> Result<usize, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(50);
    }
    t.parse::<usize>()
        .map_err(|_| JsValue::from_str("max_errors must be a whole number (1 or more)"))
}

#[wasm_bindgen]
pub fn run(
    schema: &str,
    records: &str,
    input_format: &str,
    draft: &str,
    max_errors: &str,
    output: &str,
) -> Result<String, JsValue> {
    let max_errors = parse_max_errors(max_errors)?;
    gizza_ai_json_schema_batch_validate_core::render(
        schema,
        records,
        input_format,
        draft,
        max_errors,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
