//! Browser-facing wasm-bindgen wrapper for /tools/fuzzy-match/.
use gizza_ai_fuzzy_match_core::run as run_core;
use wasm_bindgen::prelude::*;

fn flag(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_i64(s: &str, fallback: i64, name: &str) -> Result<i64, JsValue> {
    let t = s.trim().replace([',', '_'], "");
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>().map_err(|_| {
        JsValue::from_str(&format!(
            "{name} must be a whole number, got `{}`",
            s.trim()
        ))
    })
}

fn parse_f64(s: &str, fallback: f64, name: &str) -> Result<f64, JsValue> {
    let t = s.trim().replace([',', '_'], "");
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{}`", s.trim())))
}

fn default_text(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

#[wasm_bindgen]
pub fn run(
    query: &str,
    candidates: &str,
    algorithm: &str,
    limit: &str,
    threshold: &str,
    case_sensitive: &str,
    include_reasons: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    run_core(
        query,
        candidates,
        &default_text(algorithm, "hybrid"),
        parse_i64(limit, 10, "limit")?,
        parse_f64(threshold, 0.0, "threshold")?,
        flag(case_sensitive, false),
        flag(include_reasons, true),
        &default_text(output_format, "text"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
