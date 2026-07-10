//! Browser-facing wasm-bindgen wrapper for /tools/json-redact/.
//! Field order MUST match meta.toml: json, style, placeholder, detect_values, extra_keys.
use gizza_ai_json_redact_core::{parse_extra_keys, redact_json, Options, Style};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    json: &str,
    style: &str,
    placeholder: &str,
    detect_values: &str,
    extra_keys: &str,
) -> Result<String, JsValue> {
    let style = Style::parse(style).map_err(|e| JsValue::from_str(&e))?;
    let opts = Options {
        style,
        placeholder,
        // Page boolean checkbox arrives as "true"/"false"; treat positive truthy as on.
        detect_values: matches!(detect_values, "true" | "1" | "on" | "yes"),
        extra_keys: parse_extra_keys(extra_keys),
    };
    let result = redact_json(json, &opts).map_err(|e| JsValue::from_str(&e))?;
    Ok(result.redacted)
}
