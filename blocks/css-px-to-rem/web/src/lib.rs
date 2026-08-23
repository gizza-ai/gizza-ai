//! Browser-facing wasm-bindgen wrapper for /tools/css-px-to-rem/.
//! Field order MUST match meta.toml: css, direction, root_font_size, precision,
//! properties, min_pixel_value, media_queries, ignore_selectors, keep_fallback,
//! unitless_zero. Fields arrive as strings; booleans as "true"/"false" (a blank
//! string means the field was absent → fall back to the schema default).
use gizza_ai_css_px_to_rem_core::{convert, Direction, Options};
use wasm_bindgen::prelude::*;

/// Positive-truthy checkbox parse (`"true"`/`"1"`/`"on"`/`"yes"`).
fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// A blank field keeps the descriptor default; anything unparseable is an error
/// the page shows verbatim rather than silently using a different number.
fn parse_number(s: &str, field: &str, default: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("invalid {field} `{t}`: expected a number")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    css: &str,
    direction: &str,
    root_font_size: &str,
    precision: &str,
    properties: &str,
    min_pixel_value: &str,
    media_queries: &str,
    ignore_selectors: &str,
    keep_fallback: &str,
    unitless_zero: &str,
) -> Result<String, JsValue> {
    let direction = Direction::parse(Some(direction)).map_err(|e| JsValue::from_str(&e))?;
    let precision = parse_number(precision, "precision", 5.0)?;
    if !(0.0..=10.0).contains(&precision) {
        return Err(JsValue::from_str(&format!(
            "invalid precision `{precision}`: expected 0-10 decimal places"
        )));
    }
    let opts = Options {
        direction,
        root_font_size: parse_number(root_font_size, "root_font_size", 16.0)?,
        precision: precision as usize,
        properties: if properties.trim().is_empty() {
            "*".to_string()
        } else {
            properties.to_string()
        },
        min_pixel_value: parse_number(min_pixel_value, "min_pixel_value", 0.0)?,
        media_queries: truthy(media_queries),
        ignore_selectors: ignore_selectors.to_string(),
        keep_fallback: truthy(keep_fallback),
        // Default-true checkbox: a blank string only happens if the field is
        // absent — treat that as the default (on).
        unitless_zero: unitless_zero.trim().is_empty() || truthy(unitless_zero),
    };
    convert(css, &opts).map_err(|e| JsValue::from_str(&e))
}
