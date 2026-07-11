//! Browser-facing wasm-bindgen wrapper for /tools/sql-dump-to-csv/.
//! Compiled with wasm-pack for the standalone /tools/sql-dump-to-csv/ page.
use wasm_bindgen::prelude::*;

/// Convert a SQL dump into CSV (one section per table).
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `sql`: the dump text (required).
/// - `table`: export only this table (blank → all).
/// - `delimiter`: `comma`/`tab`/`semicolon`/`pipe` (blank → comma).
/// - `header`: `"true"`/`"1"`/`"yes"`/`"on"` → include the column-name row;
///   anything else → omit it. (A default-checked checkbox sends `"true"`.)
/// - `null_value`: text written for a SQL NULL cell (blank → empty field).
/// - `quote`: `minimal`/`all` (blank → minimal).
/// - `bom`: `"true"`/`"1"`/`"yes"`/`"on"` → prepend a UTF-8 BOM; else off.
///
/// Throws a JS error string on an unknown delimiter/quote mode or when no
/// INSERT statements are found.
#[wasm_bindgen]
pub fn run(
    sql: &str,
    table: &str,
    delimiter: &str,
    header: &str,
    null_value: &str,
    quote: &str,
    bom: &str,
) -> Result<String, JsValue> {
    let header = truthy(header);
    let bom = truthy(bom);
    gizza_ai_sql_dump_to_csv_core::convert(sql, table, delimiter, header, null_value, quote, bom)
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
