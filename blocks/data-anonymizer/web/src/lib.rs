//! Browser-facing wasm-bindgen wrapper for /tools/data-anonymizer/.
//! Field order MUST match meta.toml: data, quasi, identifiers, sensitive, k,
//! numeric_bin, text_keep, dates_to_year, suppress, header, delimiter, label,
//! output.
use gizza_ai_data_anonymizer_core::{anonymize_csv, Options, Output};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Empty / unparsable strings fall back to the descriptor defaults; the core
/// still validates ranges (k >= 2, numeric_bin > 0).
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    quasi: &str,
    identifiers: &str,
    sensitive: &str,
    k: &str,
    numeric_bin: &str,
    text_keep: &str,
    dates_to_year: &str,
    suppress: &str,
    header: &str,
    delimiter: &str,
    label: &str,
    output: &str,
) -> Result<String, JsValue> {
    // default-true booleans: only an explicit false-y value turns them off.
    let dates = !matches!(
        dates_to_year.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let hdr = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let opts = Options {
        k: k.trim().parse::<usize>().unwrap_or(2),
        numeric_bin: numeric_bin.trim().parse::<f64>().unwrap_or(10.0),
        text_keep: text_keep.trim().parse::<usize>().unwrap_or(3),
        dates_to_year: dates,
        suppress: truthy(suppress),
        label: if label.is_empty() {
            "[REDACTED]".to_string()
        } else {
            label.to_string()
        },
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    anonymize_csv(data, quasi, identifiers, sensitive, hdr, delim, &opts)
        .map_err(|e| JsValue::from_str(&e))
}
