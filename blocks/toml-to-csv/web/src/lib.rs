//! Browser-facing wasm-bindgen wrapper for /tools/toml-to-csv/.
//! Compiled with wasm-pack for the standalone /tools/toml-to-csv/ page.
//! Every field value arrives as a string; the core validates the enum params
//! (returning a JS error on a bad value). Argument order mirrors the descriptor.
use wasm_bindgen::prelude::*;

/// Convert a TOML array-of-tables into CSV rows.
///
/// - `input`: the TOML document.
/// - `table`: dotted path of the array-of-tables (empty = auto-detect).
/// - `nested`: `flatten` / `json` / `skip` for nested tables.
/// - `array_format`: `json` / `join` / `columns` for array values.
/// - `columns`: `union` / `sorted` / `first` header ordering.
/// - `delimiter`: `comma` / `semicolon` / `tab` / `pipe`.
/// - `include_header`: checkbox string — `"true"`/`"1"`/`"on"`/`"yes"` is on.
///
/// Throws a JS error string on an unknown enum value or unparsable TOML.
#[wasm_bindgen]
pub fn run(
    input: &str,
    table: &str,
    nested: &str,
    array_format: &str,
    columns: &str,
    delimiter: &str,
    include_header: &str,
) -> Result<String, JsValue> {
    let header = matches!(
        include_header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_toml_to_csv_core::convert(
        input,
        table,
        nested,
        array_format,
        columns,
        delimiter,
        header,
    )
    .map_err(|e| JsValue::from_str(&e))
}
