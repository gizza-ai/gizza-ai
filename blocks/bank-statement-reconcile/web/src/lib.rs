//! Browser-facing wasm-bindgen wrapper for /tools/bank-statement-reconcile/.
use wasm_bindgen::prelude::*;

fn parse_i64_default(v: &str, default: i64, name: &str) -> Result<i64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<i64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be an integer")))
    }
}
fn parse_f64_default(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}
fn parse_u8_default(v: &str, default: u8, name: &str) -> Result<u8, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<u8>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be an integer 0-100")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    statement_csv: &str,
    ledger_csv: &str,
    stmt_date: &str,
    stmt_amount: &str,
    stmt_memo: &str,
    ledger_date: &str,
    ledger_amount: &str,
    ledger_memo: &str,
    date_tolerance_days: &str,
    amount_tolerance: &str,
    memo_threshold: &str,
    delimiter: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_bank_statement_reconcile_core::reconcile(
        statement_csv,
        ledger_csv,
        if stmt_date.trim().is_empty() {
            "date"
        } else {
            stmt_date
        },
        if stmt_amount.trim().is_empty() {
            "amount"
        } else {
            stmt_amount
        },
        if stmt_memo.trim().is_empty() {
            "memo"
        } else {
            stmt_memo
        },
        if ledger_date.trim().is_empty() {
            "date"
        } else {
            ledger_date
        },
        if ledger_amount.trim().is_empty() {
            "amount"
        } else {
            ledger_amount
        },
        if ledger_memo.trim().is_empty() {
            "memo"
        } else {
            ledger_memo
        },
        parse_i64_default(date_tolerance_days, 3, "date_tolerance_days")?,
        parse_f64_default(amount_tolerance, 0.01, "amount_tolerance")?,
        parse_u8_default(memo_threshold, 70, "memo_threshold")?,
        if delimiter.trim().is_empty() {
            "comma"
        } else {
            delimiter
        },
        if output.trim().is_empty() {
            "markdown"
        } else {
            output
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
