//! Browser-facing wasm-bindgen wrapper for /tools/fuzzy-csv-join/.
use gizza_ai_fuzzy_csv_join_core::fuzzy_join;
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        "false" | "0" | "off" | "no" => false,
        _ => default,
    }
}

fn parse_f64(s: &str, default: f64) -> f64 {
    s.trim().parse().unwrap_or(default)
}

fn parse_usize(s: &str, default: usize) -> usize {
    s.trim().parse().unwrap_or(default)
}

#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    left_key: &str,
    right_key: &str,
    algorithm: &str,
    threshold: &str,
    join_type: &str,
    max_matches: &str,
    show_score: &str,
    normalize_case: &str,
    ignore_punctuation: &str,
    delimiter: &str,
    output: &str,
) -> Result<String, JsValue> {
    fuzzy_join(
        left,
        right,
        left_key,
        right_key,
        if algorithm.is_empty() {
            "jaro_winkler"
        } else {
            algorithm
        },
        parse_f64(threshold, 85.0),
        if join_type.is_empty() {
            "inner"
        } else {
            join_type
        },
        parse_usize(max_matches, 1),
        parse_bool(show_score, true),
        parse_bool(normalize_case, true),
        parse_bool(ignore_punctuation, false),
        if delimiter.is_empty() { "," } else { delimiter },
        if output.is_empty() { "csv" } else { output },
    )
    .map_err(|e| JsValue::from_str(&e))
}
