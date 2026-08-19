//! Browser-facing wasm-bindgen wrapper for /tools/zero-pad-ids/.
//! Field order MUST match page/meta.toml — the page driver passes every field
//! value as a raw string, so each one is parsed here and the core owns the
//! validation.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_width(s: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>().map_err(|_| {
        JsValue::from_str("width must be a whole number between 0 and 64 (0 = auto-fit)")
    })
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    delimiter: &str,
    columns: &str,
    width: &str,
    mode: &str,
    overflow: &str,
    non_numeric: &str,
    header: &str,
    quote_style: &str,
) -> Result<String, JsValue> {
    let width = parse_width(width)?;
    gizza_ai_zero_pad_ids_core::zero_pad(
        input,
        delimiter,
        columns,
        width,
        mode,
        overflow,
        non_numeric,
        // A blank string means the page never rendered the checkbox value;
        // the descriptor default is on.
        truthy(header) || header.trim().is_empty(),
        quote_style,
    )
    .map_err(|e| JsValue::from_str(&e))
}
