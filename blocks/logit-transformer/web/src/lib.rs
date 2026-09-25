//! Browser-facing wasm-bindgen wrapper for /tools/logit-transformer/.
//! Argument order matches page/meta.toml.
use wasm_bindgen::prelude::*;

fn parse_decimals(value: &str) -> Result<Option<u32>, JsValue> {
    let v = value.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let n: u32 = v
        .parse()
        .map_err(|_| JsValue::from_str("decimals must be 'auto' or an integer from 0 to 8"))?;
    if n > 8 {
        return Err(JsValue::from_str("decimals must be 'auto' or 0-8"));
    }
    Ok(Some(n))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    direction: &str,
    base: &str,
    separator: &str,
    output_separator: &str,
    on_boundary: &str,
    epsilon: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let epsilon: f64 = epsilon.trim().parse().unwrap_or(0.000001);
    let decimals = parse_decimals(decimals)?;
    gizza_ai_logit_transformer_core::run(
        data,
        direction,
        base,
        separator,
        output_separator,
        on_boundary,
        epsilon,
        decimals,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
