//! Browser-facing wasm-bindgen wrapper for /tools/regex-from-examples/.
use wasm_bindgen::prelude::*;

fn truthy(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    examples: &str,
    negatives: &str,
    separator: &str,
    strategy: &str,
    quantifiers: &str,
    flavor: &str,
    anchors: &str,
    case_insensitive: &str,
    capture_groups: &str,
    output: &str,
    max_alternatives: &str,
) -> Result<String, JsValue> {
    let max_alternatives = max_alternatives.trim().parse::<f64>().unwrap_or(50.0);
    gizza_ai_regex_from_examples_core::render(
        examples,
        negatives,
        separator,
        strategy,
        quantifiers,
        flavor,
        truthy(anchors, true),
        truthy(case_insensitive, false),
        truthy(capture_groups, false),
        output,
        max_alternatives,
    )
    .map_err(|e| JsValue::from_str(&e))
}
