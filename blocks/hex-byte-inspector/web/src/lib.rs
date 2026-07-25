//! Browser-facing wasm-bindgen wrapper for /tools/hex-byte-inspector/.
//! Field order MUST match meta.toml: input, input_format, group_size,
//! uppercase, interpret. Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_int(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    group_size: &str,
    uppercase: &str,
    interpret: &str,
) -> Result<String, JsValue> {
    // An empty format field falls back to the schema default (hex).
    let fmt = if input_format.trim().is_empty() { "hex" } else { input_format };
    let group = parse_int(group_size, "group_size", 4)?;
    gizza_ai_hex_byte_inspector_core::inspect(input, fmt, group, truthy(uppercase), truthy(interpret))
        .map_err(|e| JsValue::from_str(&e))
}
