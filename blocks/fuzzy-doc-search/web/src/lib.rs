//! Browser-facing wasm-bindgen wrapper for /tools/fuzzy-doc-search/.
//! Compiled with wasm-pack for the standalone page. Every field value arrives
//! as a string (the page passes fields in declared order); parse/clamp here and
//! delegate to the shared pure core, returning the rendered ranked-snippet text.
use gizza_ai_fuzzy_doc_search_core::{search_text, Options, Unit};
use wasm_bindgen::prelude::*;

/// Param order MUST match `page/meta.toml` `[[input]]` order and the
/// descriptor: query, text, match, fuzziness, case_sensitive, whole_word, unit,
/// max_results.
#[wasm_bindgen]
pub fn run(
    query: &str,
    text: &str,
    r#match: &str,
    fuzziness: &str,
    case_sensitive: &str,
    whole_word: &str,
    unit: &str,
    max_results: &str,
) -> Result<String, JsValue> {
    let match_all = match r#match.trim().to_ascii_lowercase().as_str() {
        "" | "any" => false,
        "all" => true,
        other => return Err(JsValue::from_str(&format!(
            "unknown match {other:?}: expected any or all"
        ))),
    };
    let fuzziness = fuzziness.trim().parse::<usize>().unwrap_or(1);
    let max_results = max_results.trim().parse::<usize>().unwrap_or(10).max(1);
    let unit = Unit::parse(if unit.trim().is_empty() { "line" } else { unit })
        .map_err(|e| JsValue::from_str(&e))?;
    let opts = Options {
        match_all,
        fuzziness,
        case_sensitive: truthy(case_sensitive),
        whole_word: truthy(whole_word),
        unit,
        max_results,
    };
    search_text(query, text, opts).map_err(|e| JsValue::from_str(&e))
}

/// A page checkbox marshals as "true"/"false" (positive-truthy also accepts
/// "1"/"on"/"yes"); anything else is false.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}
