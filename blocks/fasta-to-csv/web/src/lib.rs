//! Browser-facing wasm-bindgen wrapper for /tools/fasta-to-csv/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the boolean/enum params are parsed here.
use gizza_ai_fasta_to_csv_core::{convert, Delimiter, HeaderMode, Options};
use wasm_bindgen::prelude::*;

/// Positive-truthy: the page sends `"true"`/`"false"` for checkboxes.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Checkbox that defaults to ON — a blank/missing value means "unchanged", i.e. on.
fn truthy_default_on(v: &str) -> bool {
    if v.trim().is_empty() {
        true
    } else {
        truthy(v)
    }
}

/// Convert FASTA text to a delimited table.
///
/// - `fasta`: the FASTA records.
/// - `delimiter`: `"comma"` (default/blank), `"tab"`, `"semicolon"` or `"pipe"`.
/// - `header_mode`: `"split"` (default/blank), `"id_only"` or `"full_header"`.
/// - `header_row` / `include_sequence` / `include_length`: default ON — blank means on.
/// - `include_gc` / `include_base_counts` / `uppercase` / `dedupe`: default OFF.
///
/// Throws a JS error string on malformed FASTA or an invalid enum value.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    fasta: &str,
    delimiter: &str,
    header_mode: &str,
    header_row: &str,
    include_sequence: &str,
    include_length: &str,
    include_gc: &str,
    include_base_counts: &str,
    uppercase: &str,
    dedupe: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        delimiter: Delimiter::parse(delimiter).map_err(|e| JsValue::from_str(&e))?,
        header_mode: HeaderMode::parse(header_mode).map_err(|e| JsValue::from_str(&e))?,
        header_row: truthy_default_on(header_row),
        include_sequence: truthy_default_on(include_sequence),
        include_length: truthy_default_on(include_length),
        include_gc: truthy(include_gc),
        include_base_counts: truthy(include_base_counts),
        uppercase: truthy(uppercase),
        dedupe: truthy(dedupe),
    };
    convert(fasta, &opts).map_err(|e| JsValue::from_str(&e))
}
