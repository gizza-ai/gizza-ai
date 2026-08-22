//! Browser-facing wasm-bindgen wrapper for /tools/yaml-query/.
//! Field order MUST match page/meta.toml: yaml, query, input_format,
//! output_format, documents, pretty, raw_output.
use gizza_ai_yaml_query_core::{run_query_text, DocMode, InFormat, Options, OutFormat};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    yaml: &str,
    query: &str,
    input_format: &str,
    output_format: &str,
    documents: &str,
    pretty: &str,
    raw_output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        input_format: InFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        output_format: OutFormat::parse(output_format).map_err(|e| JsValue::from_str(&e))?,
        documents: DocMode::parse(documents).map_err(|e| JsValue::from_str(&e))?,
        pretty: truthy(pretty),
        raw_output: truthy(raw_output),
    };
    run_query_text(yaml, query, &opts).map_err(|e| JsValue::from_str(&e))
}
