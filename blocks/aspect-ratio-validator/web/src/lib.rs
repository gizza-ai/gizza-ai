//! Browser-facing wasm-bindgen wrapper for /tools/aspect-ratio-validator/.
//!
//! Argument order MUST match page/meta.toml. Every field arrives as a string
//! (the page driver does no coercion for pure tools), and a blank one falls
//! back to the same default the descriptor declares — so the page, the chat
//! schema and the CLI all agree.
use gizza_ai_aspect_ratio_validator_core::{analyze_json, Options, DEFAULT_TOLERANCE_PERCENT};
use wasm_bindgen::prelude::*;

/// Parse a required number field with a field-specific message.
fn parse_required(s: &str, label: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(format!("enter the {label} in pixels (for example 1920)"));
    }
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number of pixels, got {t:?}"))
}

/// Parse an optional number field, falling back to `default` when blank.
fn parse_optional(s: &str, default: f64, label: &str) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number, got {t:?}"))
}

/// Checkboxes arrive as "true"/"false"; treat every positive spelling as on.
fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    width: &str,
    height: &str,
    target: &str,
    tolerance_percent: &str,
    orientation_agnostic: &str,
    even_dimensions: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        width: parse_required(width, "width").map_err(|e| JsValue::from_str(&e))?,
        height: parse_required(height, "height").map_err(|e| JsValue::from_str(&e))?,
        target: target.trim().to_string(),
        tolerance_percent: parse_optional(
            tolerance_percent,
            DEFAULT_TOLERANCE_PERCENT,
            "tolerance_percent",
        )
        .map_err(|e| JsValue::from_str(&e))?,
        orientation_agnostic: truthy(orientation_agnostic),
        even_dimensions: truthy(even_dimensions),
    };
    analyze_json(&opts).map_err(|e| JsValue::from_str(&e))
}
