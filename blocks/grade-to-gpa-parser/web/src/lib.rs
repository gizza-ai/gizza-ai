//! Browser-facing wasm-bindgen wrapper for /tools/grade-to-gpa-parser/.
//! Field order MUST match meta.toml: grades, scale, custom_scale, grade_format,
//! default_credits, honors_bonus, ap_bonus, prior_gpa, prior_credits,
//! skip_nongraded, decimals, output. The page marshals every field as a string,
//! so the numeric and boolean ones are parsed here.
use gizza_ai_grade_to_gpa_parser_core::{summary, Options};
use wasm_bindgen::prelude::*;

/// Parse a numeric field, falling back to `fallback` when it is left blank.
fn num(v: &str, fallback: f64, field: &str) -> Result<f64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{field} must be a number, got '{v}'"))
}

/// Parse a whole-number field with an inclusive range check.
fn int(v: &str, fallback: i64, lo: i64, hi: i64, field: &str) -> Result<i64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    let n: i64 = t
        .parse()
        .map_err(|_| format!("{field} must be a whole number, got '{v}'"))?;
    if n < lo || n > hi {
        return Err(format!("{field} must be between {lo} and {hi} (got {n})"));
    }
    Ok(n)
}

/// A checkbox arrives as "true"/"false"; anything positive-truthy counts as on.
fn flag(v: &str, fallback: bool) -> bool {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() {
        return fallback;
    }
    matches!(t.as_str(), "true" | "1" | "yes" | "on")
}

/// Fall back to a default when the page sends an empty select/text field.
fn word(v: &str, fallback: &str) -> String {
    let t = v.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    grades: &str,
    scale: &str,
    custom_scale: &str,
    grade_format: &str,
    default_credits: &str,
    honors_bonus: &str,
    ap_bonus: &str,
    prior_gpa: &str,
    prior_credits: &str,
    skip_nongraded: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let inner = || -> Result<String, String> {
        let opts = Options {
            scale: word(scale, "4.0"),
            custom_scale: custom_scale.to_string(),
            grade_format: word(grade_format, "auto"),
            default_credits: num(default_credits, 1.0, "default credits")?,
            honors_bonus: num(honors_bonus, 0.5, "honors bonus")?,
            ap_bonus: num(ap_bonus, 1.0, "AP bonus")?,
            prior_gpa: num(prior_gpa, 0.0, "prior GPA")?,
            prior_credits: num(prior_credits, 0.0, "prior credits")?,
            skip_nongraded: flag(skip_nongraded, true),
            decimals: int(decimals, 2, 0, 6, "decimals")? as u32,
            output: word(output, "report"),
        };
        summary(grades, &opts)
    };
    inner().map_err(|e| JsValue::from_str(&e))
}
