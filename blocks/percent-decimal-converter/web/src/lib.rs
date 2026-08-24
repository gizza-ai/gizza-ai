//! Browser-facing wasm-bindgen wrapper for /tools/percent-decimal-converter/.
//! Field order MUST match page/meta.toml: data, direction, unit, columns,
//! header, delimiter, decimals, trim_zeros, suffix. Every field arrives as a
//! string (checkboxes send "true"/"false"); the core owns all validation and
//! error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Convert the chosen columns between percentages and decimal fractions.
///
/// - `data`: the table or one-value-per-line list (max 5,000,000 bytes).
/// - `direction`: `auto` | `percent_to_decimal` | `decimal_to_percent`.
/// - `unit`: `percent` | `permille` | `basis_points`.
/// - `columns`: blank for every column, else names / 1-based indices, comma-separated.
/// - `header`: checkbox `"true"`/`"false"` (default-checked).
/// - `delimiter`: `comma`/`tab`/`semicolon`/`pipe` or any single character.
/// - `decimals`: `-1` (exact, the default) or `0`..=`12` fixed places.
/// - `trim_zeros`: checkbox — drop trailing zeros after rounding.
/// - `suffix`: checkbox `"true"`/`"false"` (default-checked) — append %/‰/bps.
///
/// Throws a JS error string on empty or over-cap input, a non-integer or
/// out-of-range `decimals`, an unknown option, or an unknown/out-of-range column.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    direction: &str,
    unit: &str,
    columns: &str,
    header: &str,
    delimiter: &str,
    decimals: &str,
    trim_zeros: &str,
    suffix: &str,
) -> Result<String, JsValue> {
    let trimmed = decimals.trim();
    let decimals: i64 = if trimmed.is_empty() {
        -1
    } else {
        trimmed.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "decimals must be a whole number between -1 (exact) and 12, got '{trimmed}'"
            ))
        })?
    };
    gizza_ai_percent_decimal_converter_core::convert_csv(
        data,
        direction,
        unit,
        columns,
        truthy(header),
        delimiter,
        decimals,
        truthy(trim_zeros),
        truthy(suffix),
    )
    .map_err(|e| JsValue::from_str(&e))
}
