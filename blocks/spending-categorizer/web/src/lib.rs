//! Browser-facing wasm-bindgen wrapper for /tools/spending-categorizer/.
//! Compiled with wasm-pack for the standalone page. Every field value arrives
//! as a string; `invert_amount` is parsed from its checkbox string here, and
//! the core validates the enum params (returning a JS error on a bad value).
use wasm_bindgen::prelude::*;

/// Auto-categorize a bank/credit-card CSV export and summarize spending.
///
/// - `data`: the pasted CSV, with a header row.
/// - `description_column` / `amount_column` / `debit_column` / `credit_column`
///   / `date_column`: column names (blank = auto-detect).
/// - `rules`: newline `keyword = Category` rules, checked before built-ins.
/// - `output`: `both` / `summary` / `csv`.
/// - `currency`: symbol (`$`, prefixed) or code (`USD`, suffixed).
/// - `delimiter`: `auto` / `comma` / `semicolon` / `tab` / `pipe`.
/// - `invert_amount`: `"true"`/`"1"` to flip the sign of every amount.
///
/// Throws a JS error string on an unknown enum value or unparsable input.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    description_column: &str,
    amount_column: &str,
    debit_column: &str,
    credit_column: &str,
    date_column: &str,
    rules: &str,
    output: &str,
    currency: &str,
    delimiter: &str,
    invert_amount: &str,
) -> Result<String, JsValue> {
    let invert = matches!(
        invert_amount.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_spending_categorizer_core::categorize_spending(
        data,
        description_column,
        amount_column,
        debit_column,
        credit_column,
        date_column,
        rules,
        output,
        currency,
        delimiter,
        invert,
    )
    .map_err(|e| JsValue::from_str(&e))
}
