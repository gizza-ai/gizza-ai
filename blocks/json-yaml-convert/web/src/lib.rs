//! Browser-facing wasm-bindgen wrapper for /tools/json-yaml-convert/.
//! Field order MUST match meta.toml: input, from, to, pretty. Fields are strings.
use gizza_ai_json_yaml_convert_core::{convert, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, from: &str, to: &str, pretty: &str) -> Result<String, JsValue> {
    let f = Format::parse(from).map_err(|e| JsValue::from_str(&e))?;
    let t = Format::parse(to).map_err(|e| JsValue::from_str(&e))?;
    // pretty checkbox defaults checked; only an explicit off value disables it.
    let p = !matches!(pretty.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    convert(input, f, t, p).map_err(|e| JsValue::from_str(&e))
}
