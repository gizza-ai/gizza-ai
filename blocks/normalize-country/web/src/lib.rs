//! Browser-facing wasm-bindgen wrapper for /tools/normalize-country/.
//!
//! The page driver hands every field through as a raw string, so each param is
//! taken as `&str` and parsed here; the core owns all validation so the page,
//! the CLI and chat funnel through exactly the same rules.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; treat any positive spelling as on.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    output: &str,
    name_style: &str,
    delimiter: &str,
    on_unmatched: &str,
    dedupe: &str,
    sort: &str,
    fuzzy: &str,
) -> Result<String, JsValue> {
    gizza_ai_normalize_country_core::normalize(
        input,
        output,
        name_style,
        delimiter,
        on_unmatched,
        truthy(dedupe, false),
        sort,
        truthy(fuzzy, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
