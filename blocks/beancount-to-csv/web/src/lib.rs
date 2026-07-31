//! Browser-facing wasm-bindgen wrapper for /tools/beancount-to-csv/.
//! Compiled with wasm-pack for the standalone /tools/beancount-to-csv/ page.
//! Every field value arrives as a string; the core validates the enum params
//! (returning a JS error on a bad value). Argument order mirrors the descriptor.
use wasm_bindgen::prelude::*;

/// Convert between a Beancount/Ledger journal and a flat CSV of postings.
///
/// - `direction`: `to-csv` (journal → CSV) or `from-csv` (CSV → journal).
/// - `input`: the journal text (to-csv) or CSV text with a header row (from-csv).
/// - `journal_format`: `beancount` / `ledger` — dialect written by from-csv.
/// - `delimiter`: `comma` / `semicolon` / `tab` / `pipe` CSV separator.
///
/// Throws a JS error string on an unknown enum value or unparsable input.
#[wasm_bindgen]
pub fn run(
    direction: &str,
    input: &str,
    journal_format: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    gizza_ai_beancount_to_csv_core::convert(input, direction, journal_format, delimiter)
        .map_err(|e| JsValue::from_str(&e))
}
