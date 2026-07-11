//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-vcard/.
//! Compiled with wasm-pack for the standalone /tools/csv-to-vcard/ page. Each
//! field value arrives as a string; the enum params are passed straight through
//! to the core (which validates them and returns a JS error on a bad value).
use wasm_bindgen::prelude::*;

/// Convert a CSV/JSON contact list into a `.vcf` file of vCards.
///
/// - `data`: the pasted CSV (with a header row) or JSON array of contacts.
/// - `input_format`: `"auto"` (default) / `"csv"` / `"json"`.
/// - `delimiter`: `"auto"` (default) / `"comma"` / `"semicolon"` / `"tab"` / `"pipe"`.
/// - `version`: `"3.0"` (default) / `"4.0"`.
///
/// Throws a JS error string on an unknown enum value or unparsable input.
#[wasm_bindgen]
pub fn run(
    data: &str,
    input_format: &str,
    delimiter: &str,
    version: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_to_vcard_core::to_vcards(data, input_format, delimiter, version)
        .map_err(|e| JsValue::from_str(&e))
}
