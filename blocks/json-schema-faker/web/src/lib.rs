//! Browser-facing wasm-bindgen wrapper for /tools/json-schema-faker/.
use wasm_bindgen::prelude::*;

fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    schema: &str,
    count: &str,
    seed: &str,
    pretty: &str,
    output: &str,
) -> Result<String, JsValue> {
    let count = if count.trim().is_empty() {
        1
    } else {
        count
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("count must be an integer"))?
    };
    let seed = if seed.trim().is_empty() {
        1
    } else {
        seed.trim()
            .parse::<u64>()
            .map_err(|_| JsValue::from_str("seed must be an integer"))?
    };
    let output = if output.trim().is_empty() {
        "json"
    } else {
        output.trim()
    };
    gizza_ai_json_schema_faker_core::generate(
        schema,
        count,
        seed,
        truthy_default_on(pretty),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
