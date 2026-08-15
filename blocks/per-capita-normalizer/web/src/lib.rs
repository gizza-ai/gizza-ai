//! Browser-facing wasm-bindgen wrapper for /tools/per-capita-normalizer/.
//! The page driver passes every field as a string, so numeric params are parsed
//! here and all validation stays in the shared core.
use wasm_bindgen::prelude::*;

fn parse_num(v: &str, fallback: f64, name: &str) -> Result<f64, JsValue> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got '{t}'")))
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v.trim()
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    per: &str,
    custom_per: &str,
    population_unit: &str,
    decimals: &str,
    sort: &str,
    unstable_below: &str,
    output: &str,
) -> Result<String, JsValue> {
    let custom_per = parse_num(custom_per, 0.0, "custom_per")?;
    let decimals = parse_num(decimals, 2.0, "decimals")?;
    let unstable_below = parse_num(unstable_below, 20.0, "unstable_below")?;
    if decimals.fract() != 0.0 || !(0.0..=6.0).contains(&decimals) {
        return Err(JsValue::from_str(
            "decimals must be a whole number between 0 and 6",
        ));
    }
    if unstable_below.fract() != 0.0 || !(0.0..=1_000_000.0).contains(&unstable_below) {
        return Err(JsValue::from_str(
            "unstable_below must be a whole number between 0 and 1000000",
        ));
    }
    gizza_ai_per_capita_normalizer_core::run(
        data,
        or_default(delimiter, "auto"),
        or_default(header, "auto"),
        or_default(per, "100000"),
        custom_per,
        or_default(population_unit, "ones"),
        decimals as usize,
        or_default(sort, "rate_desc"),
        unstable_below,
        or_default(output, "table"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
