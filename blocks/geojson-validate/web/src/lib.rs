//! Browser-facing wasm-bindgen wrapper for /tools/geojson-validate/.
//! Field order MUST match meta.toml: input, output, strict_winding, allow_bbox,
//! warn_problematic, max_precision, max_issues.
use wasm_bindgen::prelude::*;

fn parse_i64(raw: &str, default: i64, name: &str) -> Result<i64, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<i64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

/// Checkboxes arrive as "true"/"false"; an empty field means "left at default".
fn truthy_default_true(raw: &str) -> bool {
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    strict_winding: &str,
    allow_bbox: &str,
    warn_problematic: &str,
    max_precision: &str,
    max_issues: &str,
) -> Result<String, JsValue> {
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output
    };
    let max_precision = parse_i64(max_precision, 6, "max_precision")?;
    let max_issues = parse_i64(max_issues, 50, "max_issues")?;

    gizza_ai_geojson_validate_core::run(
        input,
        output,
        truthy_default_true(strict_winding),
        truthy_default_true(allow_bbox),
        truthy_default_true(warn_problematic),
        max_precision,
        max_issues,
    )
    .map_err(|e| JsValue::from_str(&e))
}
