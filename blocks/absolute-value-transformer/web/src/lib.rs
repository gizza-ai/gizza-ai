//! Browser-facing wasm-bindgen wrapper for /tools/absolute-value-transformer/.
use wasm_bindgen::prelude::*;

/// `auto` (full precision) or a 0-6 decimal-place count.
fn parse_decimals(s: &str) -> Result<Option<u32>, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| format!("decimals must be auto or an integer 0-6 (got {t:?})"))?;
    if n > 6 {
        return Err(format!("decimals must be 0-6 (got {n})"));
    }
    Ok(Some(n))
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    separator: &str,
    output_separator: &str,
    decimals: &str,
    on_error: &str,
    output: &str,
    stats: &str,
) -> Result<String, JsValue> {
    let decimals = parse_decimals(decimals).map_err(|e| JsValue::from_str(&e))?;
    let stats = matches!(
        stats.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_absolute_value_transformer_core::run(
        data,
        operation,
        separator,
        output_separator,
        decimals,
        on_error,
        output,
        stats,
    )
    .map_err(|e| JsValue::from_str(&e))
}
