//! Browser-facing wasm-bindgen wrapper for /tools/ledger-register/.
//! Compiled with wasm-pack for the standalone /tools/ledger-register/ page.
//! Every field value arrives as a string, so the four checkboxes and the three
//! numeric fields are parsed here; the core validates the enum params
//! (returning a JS error on a bad value).
use wasm_bindgen::prelude::*;

/// Parse a checkbox field. The page sends `"true"`/`"false"`.
fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Parse a numeric field, falling back to `fallback` when it is blank.
fn num(v: &str, name: &str, fallback: usize) -> Result<usize, JsValue> {
    match v.trim() {
        "" => Ok(fallback),
        d => Ok(d
            .parse::<i64>()
            .map_err(|_| JsValue::from_str(&format!("{name} '{d}' is not a whole number")))?
            .max(0) as usize),
    }
}

/// List the postings of a ledger-cli / hledger journal with a running total.
///
/// - `journal`: the pasted plain-text journal.
/// - `account_filter` / `payee_filter`: comma-separated substrings; `not:`/`-` excludes.
/// - `begin` / `end`: `YYYY-MM-DD` range, `end` exclusive.
/// - `status`: `all` / `cleared` / `pending` / `unmarked`.
/// - `depth`: shorten account names to N levels (`0`/blank = full name).
/// - `running_total`: `period` / `historical` / `average` / `none`.
/// - `related` / `invert` / `real_only` / `cost_basis`: checkbox strings
///   (`"true"`/`"false"`).
/// - `sort`: `date` / `date-desc` / `amount` / `amount-asc` / `account`.
/// - `limit`: max rows (`0`/blank = all); `limit_from`: `first` / `last`.
/// - `width`: text-report width in columns (blank = 80).
/// - `output_format`: `text` / `csv` / `json` / `markdown`.
///
/// Throws a JS error string on an unknown enum value or an unparsable journal.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    journal: &str,
    account_filter: &str,
    payee_filter: &str,
    begin: &str,
    end: &str,
    status: &str,
    depth: &str,
    running_total: &str,
    related: &str,
    invert: &str,
    real_only: &str,
    cost_basis: &str,
    sort: &str,
    limit: &str,
    limit_from: &str,
    width: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let depth_n = num(depth, "depth", 0)?;
    let limit_n = num(limit, "limit", 0)?;
    let width_n = num(width, "width", 80)?;
    gizza_ai_ledger_register_core::register(
        journal,
        account_filter,
        payee_filter,
        begin,
        end,
        status,
        depth_n,
        running_total,
        flag(related),
        flag(invert),
        flag(real_only),
        flag(cost_basis),
        sort,
        limit_n,
        limit_from,
        width_n,
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
