//! Browser-facing wasm-bindgen wrapper for /tools/xpath-query/.
//! Field order MUST match meta.toml: expression, xml, output.
use gizza_ai_xpath_query_core::{query_xpath, Output};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(expression: &str, xml: &str, output: &str) -> Result<String, JsValue> {
    let mode = Output::parse(output).map_err(|e| JsValue::from_str(&e))?;
    let outs = query_xpath(expression, xml, mode).map_err(|e| JsValue::from_str(&e))?;
    Ok(outs.join("\n"))
}
