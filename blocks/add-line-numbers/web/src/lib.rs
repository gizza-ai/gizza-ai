//! Browser-facing wasm-bindgen wrapper for /tools/add-line-numbers/.
//! Field order MUST match meta.toml: text, start, step, separator, pad,
//! number_nonblank. Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_add_line_numbers_core::{render, Pad};
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
    text: &str,
    start: &str,
    step: &str,
    separator: &str,
    pad: &str,
    number_nonblank: &str,
) -> Result<String, JsValue> {
    let start = parse_int(start, "start", 1)?;
    let step = parse_int(step, "step", 1)?;
    // An empty separator field falls back to the schema default (a tab).
    let sep = if separator.is_empty() { "\t" } else { separator };
    let p = Pad::parse(pad).map_err(|e| JsValue::from_str(&e))?;
    render(text, start, step, sep, p, truthy(number_nonblank)).map_err(|e| JsValue::from_str(&e))
}
