//! Browser-facing wasm-bindgen wrapper for /tools/repeated-word-remover/.
//! Field order MUST match page/meta.toml and the descriptor: input, output,
//! keep_words, case_sensitive, across_line_breaks, ignore_punctuation,
//! include_numbers, min_length. Fields arrive as strings (checkboxes send
//! "true"/"false"). An emptied keep-list field means "protect nothing" — it is
//! deliberately NOT backfilled with the default list.
use gizza_ai_repeated_word_remover_core::{analyze, parse_keep_words, OutputFormat, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_min_length(s: &str) -> Result<usize, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(1);
    }
    t.parse::<usize>()
        .map_err(|_| JsValue::from_str("min_length must be a whole number between 1 and 20"))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    keep_words: &str,
    case_sensitive: &str,
    across_line_breaks: &str,
    ignore_punctuation: &str,
    include_numbers: &str,
    min_length: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        case_sensitive: truthy(case_sensitive),
        across_line_breaks: truthy(across_line_breaks),
        ignore_punctuation: truthy(ignore_punctuation),
        include_numbers: truthy(include_numbers),
        min_length: parse_min_length(min_length)?,
        keep_words: parse_keep_words(keep_words),
        format: OutputFormat::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    analyze(input, &opts)
        .map(|a| a.output)
        .map_err(|e| JsValue::from_str(&e))
}
