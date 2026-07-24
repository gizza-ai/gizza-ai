//! Browser-facing wasm-bindgen wrapper for /tools/survey-tabulator/.
//! Field order MUST match page/meta.toml: data, mode, question, by, percent,
//! include_blanks, stats, sort, top, delimiter. Each value arrives as a string
//! (checkboxes send "true"/"false"); blanks fall through to the core defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Parse the optional integer `top` field: blank → 0 (all categories).
fn parse_top(s: &str) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("top must be a whole number (got '{t}')")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    mode: &str,
    question: &str,
    by: &str,
    percent: &str,
    include_blanks: &str,
    stats: &str,
    sort: &str,
    top: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let top = parse_top(top)?;
    gizza_ai_survey_tabulator_core::tabulate(
        data,
        mode,
        question,
        by,
        percent,
        truthy(include_blanks),
        truthy(stats),
        sort,
        top,
        delimiter,
    )
    .map_err(|e| JsValue::from_str(&e))
}
