//! Browser-facing wasm-bindgen wrapper for /tools/human-readable-units/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the numeric/boolean/enum params are parsed here.
use gizza_ai_human_readable_units_core::{
    format, parse_base, parse_detail, parse_input_unit, parse_kind, parse_style, Options,
};
use wasm_bindgen::prelude::*;

/// Page checkboxes send "true"/"false"; be generous about the other truthy forms.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// A checkbox whose descriptor default is `true`: an empty value means "not
/// supplied" (deep link without the param), which must keep the default on.
fn truthy_default_on(v: &str) -> bool {
    if v.trim().is_empty() {
        true
    } else {
        truthy(v)
    }
}

/// A number field left blank (or holding junk) falls back to its descriptor
/// default rather than silently becoming 0.
fn u32_or(v: &str, fallback: u32) -> u32 {
    v.trim().parse::<u32>().unwrap_or(fallback)
}

/// Format a raw magnitude as a readable unit string.
///
/// - `value`: the raw number (separators, scientific notation and a sign are allowed).
/// - `kind`: `"bytes"` (default/blank), `"duration"`, `"number"`.
/// - `input_unit`: `"auto"` (default/blank) or an explicit data/time unit.
/// - `base`: `"decimal"` (default/blank), `"binary-iec"`, `"binary-jedec"`.
/// - `style`: `"short"` (default/blank), `"long"`, `"narrow"`.
/// - `precision`: decimals, 0–6 (blank → 1).
/// - `max_units`: compound duration segments, 1–7 (blank → 2).
/// - `trim_zeros`: blank → on (the descriptor default); `"false"` → keep fixed decimals.
/// - `detail`: `"value"` (default/blank), `"breakdown"`.
///
/// Throws a JS error string on an invalid option value or an unparseable number.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    value: &str,
    kind: &str,
    input_unit: &str,
    base: &str,
    style: &str,
    precision: &str,
    max_units: &str,
    trim_zeros: &str,
    detail: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        kind: parse_kind(kind).map_err(|e| JsValue::from_str(&e))?,
        input_unit: parse_input_unit(input_unit).map_err(|e| JsValue::from_str(&e))?,
        base: parse_base(base).map_err(|e| JsValue::from_str(&e))?,
        style: parse_style(style).map_err(|e| JsValue::from_str(&e))?,
        precision: u32_or(precision, 1),
        max_units: u32_or(max_units, 2),
        trim_zeros: truthy_default_on(trim_zeros),
        detail: parse_detail(detail).map_err(|e| JsValue::from_str(&e))?,
    };
    format(value, &opts).map_err(|e| JsValue::from_str(&e))
}
