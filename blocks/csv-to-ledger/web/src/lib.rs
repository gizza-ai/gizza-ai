//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-ledger/.
//! Compiled with wasm-pack for the standalone /tools/csv-to-ledger/ page. Every
//! field value arrives as a string; `invert_amount` is parsed from its checkbox
//! string here, and the core validates the enum params (returning a JS error on
//! a bad value).
use wasm_bindgen::prelude::*;

/// Convert a bank/credit-card CSV export into ledger/hledger journal entries.
///
/// - `data`: the pasted CSV, with a header row.
/// - `date_column` / `description_column` / `amount_column`: column names
///   (blank = auto-detect).
/// - `debit_column` / `credit_column`: separate money-out / money-in columns
///   (used when there is no single signed amount column).
/// - `asset_account`: the bank/card account the statement represents.
/// - `default_expense_account` / `default_income_account`: fallback legs.
/// - `account_rules`: newline `pattern = Account` rules for guessing accounts.
/// - `currency`: commodity symbol (`$`) or code (`USD`).
/// - `date_format`: `auto` / `ymd` / `mdy` / `dmy`.
/// - `output_format`: `ledger` / `hledger`.
/// - `delimiter`: `auto` / `comma` / `semicolon` / `tab` / `pipe`.
/// - `invert_amount`: `"true"`/`"1"` to flip the sign of every amount.
///
/// Throws a JS error string on an unknown enum value or unparsable input.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    date_column: &str,
    description_column: &str,
    amount_column: &str,
    debit_column: &str,
    credit_column: &str,
    asset_account: &str,
    default_expense_account: &str,
    default_income_account: &str,
    account_rules: &str,
    currency: &str,
    date_format: &str,
    output_format: &str,
    delimiter: &str,
    invert_amount: &str,
) -> Result<String, JsValue> {
    let invert = matches!(
        invert_amount.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_csv_to_ledger_core::to_ledger(
        data,
        date_column,
        description_column,
        amount_column,
        debit_column,
        credit_column,
        asset_account,
        default_expense_account,
        default_income_account,
        account_rules,
        currency,
        date_format,
        output_format,
        delimiter,
        invert,
    )
    .map_err(|e| JsValue::from_str(&e))
}
