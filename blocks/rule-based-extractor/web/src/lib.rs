//! Browser-facing wasm-bindgen wrapper for /tools/rule-based-extractor/.
//! Field order MUST match meta.toml: text, rules, split, split_pattern,
//! matches, ignore_case, multiline, dotall, trim, unique, on_missing,
//! skip_empty_records, max_records, max_matches, output, pretty.
use gizza_ai_rule_based_extractor_core::extract_text;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    rules: &str,
    split: &str,
    split_pattern: &str,
    matches: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    trim: &str,
    unique: &str,
    on_missing: &str,
    skip_empty_records: &str,
    max_records: &str,
    max_matches: &str,
    output: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    extract_text(
        text,
        rules,
        split,
        split_pattern,
        matches,
        ignore_case,
        multiline,
        dotall,
        trim,
        unique,
        on_missing,
        skip_empty_records,
        max_records,
        max_matches,
        output,
        pretty,
    )
    .map_err(|e| JsValue::from_str(&e))
}
