//! Browser-facing wasm-bindgen wrapper for /tools/currency-converter/.
//! Compiled with wasm-pack for the standalone /tools/currency-converter/ page.
use wasm_bindgen::prelude::*;

/// Convert `amount` from currency `from` to `to` using the pasted `rates` table.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `amount`: the number to convert (blank/unparseable → 1).
/// - `from` / `to`: currency codes (case-insensitive).
/// - `rates`: one `FROM TO RATE` line per rate.
/// - `bidirectional`: `"true"`/`"1"`/`"yes"`/`"on"` → also use rates in reverse
///   (default on); anything else → off.
/// - `precision`: decimal places `0`–10 (blank/unparseable → 2; core clamps).
///
/// Throws a JS error string on a bad rate line, a missing conversion path, or an
/// invalid amount/currency code.
#[wasm_bindgen]
pub fn run(
    amount: &str,
    from: &str,
    to: &str,
    rates: &str,
    bidirectional: &str,
    precision: &str,
) -> Result<String, JsValue> {
    let amount = amount.trim().parse::<f64>().unwrap_or(1.0);
    let bidirectional = matches!(
        bidirectional.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let precision = precision.trim().parse::<u32>().unwrap_or(2);
    gizza_ai_currency_converter_core::run(amount, from, to, rates, bidirectional, precision)
        .map_err(|e| JsValue::from_str(&e))
}
