//! Browser-facing wasm-bindgen wrapper for /tools/sigma-rule-matcher/.
use wasm_bindgen::prelude::*;

fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    rules: &str,
    events: &str,
    min_level: &str,
    status: &str,
    output: &str,
    max_matches: &str,
    show_event: &str,
) -> Result<String, JsValue> {
    let max_matches = match max_matches.trim() {
        "" => gizza_ai_sigma_rule_matcher_core::DEFAULT_MAX_MATCHES,
        s => s.parse::<u32>().map_err(|_| {
            JsValue::from_str(&format!(
                "max_matches must be a whole number 0-10000, got \"{s}\""
            ))
        })?,
    };
    gizza_ai_sigma_rule_matcher_core::match_rules(
        rules,
        events,
        min_level,
        status,
        output,
        max_matches,
        parse_bool(show_event, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
