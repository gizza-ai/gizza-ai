//! Browser-facing wasm-bindgen wrapper for /tools/shadcn-theme-generator/.
//!
//! Field order MUST match page/meta.toml: primary, accent, neutral, format,
//! tailwind, radius, mode, charts, sidebar.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    primary: &str,
    accent: &str,
    neutral: &str,
    format: &str,
    tailwind: &str,
    radius: &str,
    mode: &str,
    charts: &str,
    sidebar: &str,
) -> Result<String, JsValue> {
    let radius = parse_radius(radius).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_shadcn_theme_generator_core::run(
        primary,
        accent,
        &defaulted(neutral, "zinc"),
        &defaulted(format, "oklch"),
        &defaulted(tailwind, "v4"),
        radius,
        &defaulted(mode, "both"),
        truthy(charts),
        truthy(sidebar),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn defaulted(s: &str, default: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    }
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_radius(s: &str) -> Result<f64, String> {
    let t = s.trim().trim_end_matches("rem").trim();
    if t.is_empty() {
        return Ok(0.625);
    }
    t.parse::<f64>()
        .map_err(|_| format!("radius must be a number of rem from 0 to 2 (got '{t}')"))
}
