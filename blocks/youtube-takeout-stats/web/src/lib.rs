//! Browser-facing wasm-bindgen wrapper for /tools/youtube-takeout-stats/.
use wasm_bindgen::prelude::*;

fn parse_f64(s: &str, default: f64) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed.parse().unwrap_or(default)
    }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    report: &str,
    top: &str,
    utc_offset: &str,
    include_ads: &str,
    include_music: &str,
    start_date: &str,
    end_date: &str,
) -> Result<String, JsValue> {
    gizza_ai_youtube_takeout_stats_core::run(
        input,
        if output.trim().is_empty() {
            "text"
        } else {
            output
        },
        if report.trim().is_empty() {
            "overview"
        } else {
            report
        },
        parse_f64(top, 10.0),
        parse_f64(utc_offset, 0.0),
        truthy(include_ads),
        truthy(include_music),
        start_date,
        end_date,
    )
    .map_err(|e| JsValue::from_str(&e))
}
