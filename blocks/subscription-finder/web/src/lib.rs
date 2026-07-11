//! Browser-facing wasm-bindgen wrapper for /tools/subscription-finder/.
//! The standalone page passes every field value as a string, so `min_occurrences`
//! arrives as a string and is parsed here (the core clamps the range).
use wasm_bindgen::prelude::*;

/// Build the recurring-charge report for the standalone page.
///
/// - `transactions`: one `date, description, amount` row per line.
/// - `min_occurrences`: a count `2`–24 (blank/unparseable → 2; core clamps).
/// - `currency`: display symbol (blank → `$`).
/// - `date_format`: `"auto"`/`"iso"`/`"us"`/`"eu"` (blank → auto).
///
/// Throws a JS error string on an invalid `date_format` or when nothing parses.
#[wasm_bindgen]
pub fn run(
    transactions: &str,
    min_occurrences: &str,
    currency: &str,
    date_format: &str,
) -> Result<String, JsValue> {
    let min_occurrences = min_occurrences
        .trim()
        .parse::<u32>()
        .unwrap_or(gizza_ai_subscription_finder_core::MIN_OCCURRENCES);
    gizza_ai_subscription_finder_core::find(transactions, min_occurrences, currency, date_format)
        .map_err(|e| JsValue::from_str(&e))
}
