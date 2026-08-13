//! Browser-facing wasm-bindgen wrapper for /tools/entropy-calculator/.
//! Field order MUST match meta.toml: text, basis, unit, scope, ignore_case,
//! ignore_whitespace, precision, show_frequencies, top_symbols.
use wasm_bindgen::prelude::*;

fn parse_usize(raw: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

fn truthy_default_false(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn truthy_default_true(raw: &str) -> bool {
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    basis: &str,
    unit: &str,
    scope: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    precision: &str,
    show_frequencies: &str,
    top_symbols: &str,
) -> Result<String, JsValue> {
    let basis = if basis.trim().is_empty() {
        "characters"
    } else {
        basis
    };
    let unit = if unit.trim().is_empty() { "bits" } else { unit };
    let scope = if scope.trim().is_empty() {
        "whole"
    } else {
        scope
    };
    gizza_ai_entropy_calculator_core::run(
        text,
        basis,
        unit,
        scope,
        truthy_default_false(ignore_case),
        truthy_default_false(ignore_whitespace),
        parse_usize(
            precision,
            gizza_ai_entropy_calculator_core::DEFAULT_PRECISION,
            "precision",
        )?,
        truthy_default_true(show_frequencies),
        parse_usize(
            top_symbols,
            gizza_ai_entropy_calculator_core::DEFAULT_TOP_SYMBOLS,
            "top_symbols",
        )?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
