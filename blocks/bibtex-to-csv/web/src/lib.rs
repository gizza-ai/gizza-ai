//! Browser-facing wasm-bindgen wrapper for /tools/bibtex-to-csv/.
//! The page driver hands every field over as a string (checkboxes arrive as
//! "true"/"false"), so booleans are parsed positive-truthy here and the rest is
//! delegated verbatim to the shared core.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    bibtex: &str,
    columns: &str,
    custom_columns: &str,
    delimiter: &str,
    header: &str,
    decode_latex: &str,
    author_format: &str,
    author_separator: &str,
    expand_strings: &str,
    sort: &str,
    bom: &str,
) -> Result<String, JsValue> {
    gizza_ai_bibtex_to_csv_core::convert_str(
        bibtex,
        columns,
        custom_columns,
        delimiter,
        truthy(header),
        truthy(decode_latex),
        author_format,
        author_separator,
        truthy(expand_strings),
        sort,
        truthy(bom),
    )
    .map_err(|e| JsValue::from_str(&e))
}
