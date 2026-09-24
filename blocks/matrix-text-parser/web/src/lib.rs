//! Browser-facing wasm-bindgen wrapper for /tools/matrix-text-parser/.
//! The page marshals each field as a string, so checkbox strings are parsed here
//! before the pure core receives typed booleans.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    matrix: &str,
    input_format: &str,
    delimiter: &str,
    output: &str,
    cells: &str,
    fractions: &str,
    header: &str,
    ragged: &str,
    fill: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent = indent.trim().parse::<f64>().unwrap_or(2.0);
    gizza_ai_matrix_text_parser_core::parse_matrix(
        matrix,
        or_default(input_format, "auto"),
        or_default(delimiter, "auto"),
        or_default(output, "json"),
        or_default(cells, "auto"),
        truthy(fractions),
        truthy(header),
        or_default(ragged, "error"),
        fill,
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}
