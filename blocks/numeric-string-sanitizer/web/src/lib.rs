//! Browser-facing wasm-bindgen wrapper for /tools/numeric-string-sanitizer/.
use wasm_bindgen::prelude::*;

fn parse_decimals(s: &str) -> Result<Option<u32>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| format!("decimals must be auto or an integer 0-12 (got {t:?})"))?;
    if n > 12 {
        return Err(format!("decimals must be 0-12 (got {n})"));
    }
    Ok(Some(n))
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    decimal_separator: &str,
    percent: &str,
    magnitude_suffixes: bool,
    parentheses_negative: bool,
    decimals: &str,
    on_error: &str,
    output: &str,
    stats: bool,
) -> Result<String, JsValue> {
    let decimals = parse_decimals(decimals).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_numeric_string_sanitizer_core::run(
        input,
        decimal_separator,
        percent,
        magnitude_suffixes,
        parentheses_negative,
        decimals,
        on_error,
        output,
        stats,
    )
    .map_err(|e| JsValue::from_str(&e))
}
