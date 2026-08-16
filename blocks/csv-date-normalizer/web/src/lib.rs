//! Browser-facing wasm-bindgen wrapper for /tools/csv-date-normalizer/.
//! Field order MUST match page/meta.toml: data, columns, format, custom_format,
//! date_order, year_pivot, excel_serial, on_error, has_header, delimiter,
//! output. Every field arrives as a string (checkboxes send "true"/"false");
//! the core owns all validation and error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Normalize the date values in the chosen CSV columns.
///
/// - `data`: the CSV/TSV table text.
/// - `columns`: header names and/or 0-based indexes, comma-separated, or `auto`.
/// - `format`: `iso-auto` | `iso-date` | `iso-datetime` | `iso-utc` |
///   `unix-seconds` | `unix-millis` | `us-date` | `eu-date` | `sql` | `compact` |
///   `rfc2822` | `custom`.
/// - `custom_format`: the strftime pattern used when `format` is `custom`.
/// - `date_order`: `auto` | `day-first` | `month-first`.
/// - `year_pivot`: two-digit-year cut-off 0-99 (blank/unparseable → `68`).
/// - `excel_serial`: checkbox `"true"`/`"false"` (default-checked).
/// - `on_error`: `keep` | `blank` | `error`.
/// - `has_header`: checkbox `"true"`/`"false"` (default-checked).
/// - `delimiter`: `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// - `output`: `csv` | `report` | `json`.
///
/// Throws a JS error string on empty input, an unknown option, or — with
/// `on_error = "error"` — the first unreadable cell.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    format: &str,
    custom_format: &str,
    date_order: &str,
    year_pivot: &str,
    excel_serial: &str,
    on_error: &str,
    has_header: &str,
    delimiter: &str,
    output: &str,
) -> Result<String, JsValue> {
    let year_pivot = match year_pivot.trim() {
        "" => 68,
        v => v.parse::<i64>().unwrap_or(68),
    };
    gizza_ai_csv_date_normalizer_core::run(
        data,
        columns,
        format,
        custom_format,
        date_order,
        year_pivot,
        truthy(excel_serial),
        on_error,
        truthy(has_header),
        delimiter,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
