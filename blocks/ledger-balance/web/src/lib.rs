//! Browser-facing wasm-bindgen wrapper for /tools/ledger-balance/.
//! Compiled with wasm-pack for the standalone /tools/ledger-balance/ page. Every
//! field value arrives as a string, so the five checkboxes and the numeric
//! `depth` are parsed here; the core validates the enum params (returning a JS
//! error on a bad value).
use wasm_bindgen::prelude::*;

/// Parse a checkbox field. The page sends `"true"`/`"false"`.
fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Total a ledger-cli / hledger journal into per-account balances.
///
/// - `journal`: the pasted plain-text journal.
/// - `account_filter`: comma-separated substrings; `not:`/`-` excludes.
/// - `depth`: fold accounts deeper than N levels (`0`/blank = no limit).
/// - `layout`: `tree` / `flat`.
/// - `sort`: `account` / `amount` / `amount-asc`.
/// - `begin` / `end`: `YYYY-MM-DD` range, `end` exclusive.
/// - `status`: `all` / `cleared` / `pending` / `unmarked`.
/// - `include_empty` / `real_only` / `cost_basis` / `show_total` / `percent`:
///   checkbox strings (`"true"`/`"false"`).
/// - `output_format`: `text` / `csv` / `json` / `markdown`.
///
/// Throws a JS error string on an unknown enum value or an unparsable journal.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    journal: &str,
    account_filter: &str,
    depth: &str,
    layout: &str,
    sort: &str,
    begin: &str,
    end: &str,
    status: &str,
    include_empty: &str,
    real_only: &str,
    cost_basis: &str,
    show_total: &str,
    percent: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let depth_n: usize = match depth.trim() {
        "" => 0,
        d => d
            .parse::<i64>()
            .map_err(|_| JsValue::from_str(&format!("depth '{d}' is not a whole number")))?
            .max(0) as usize,
    };
    gizza_ai_ledger_balance_core::balances(
        journal,
        account_filter,
        depth_n,
        layout,
        sort,
        begin,
        end,
        status,
        flag(include_empty),
        flag(real_only),
        flag(cost_basis),
        flag(show_total),
        flag(percent),
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
