//! Browser-facing wasm-bindgen wrapper for /tools/gap-finder/.
use wasm_bindgen::prelude::*;

fn parse_i64_field(s: &str, field: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Err(JsValue::from_str(&format!("{field} must be a whole number")));
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number (got {t:?})")))
}

fn parse_usize_field(s: &str, field: &str) -> Result<usize, JsValue> {
    let n = parse_i64_field(s, field)?;
    if n < 0 {
        return Err(JsValue::from_str(&format!("{field} must be positive")));
    }
    Ok(n as usize)
}

fn parse_bool_field(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    id_format: &str,
    separator: &str,
    step: &str,
    start: &str,
    end: &str,
    order: &str,
    duplicates: &str,
    output: &str,
    limit: &str,
) -> Result<String, JsValue> {
    gizza_ai_gap_finder_core::run(
        data,
        id_format,
        separator,
        parse_i64_field(step, "step")?,
        start,
        end,
        order,
        parse_bool_field(duplicates),
        output,
        parse_usize_field(limit, "limit")?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
