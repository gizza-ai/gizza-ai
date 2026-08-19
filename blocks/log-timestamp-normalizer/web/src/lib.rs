//! Browser-facing wasm-bindgen wrapper for /tools/log-timestamp-normalizer/.
//! Field order MUST match meta.toml: log, output_format, output_timezone,
//! assume_timezone, assume_year, sort, output_mode, delta, delta_format,
//! gap_threshold_seconds, unmatched, summary. Fields arrive as strings
//! (checkboxes send "true"/"false", empty means "leave it at the default").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// An empty field means "the default" — the page clears a box rather than
/// deleting the param, so the core must not see "".
fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    log: &str,
    output_format: &str,
    output_timezone: &str,
    assume_timezone: &str,
    assume_year: &str,
    sort: &str,
    output_mode: &str,
    delta: &str,
    delta_format: &str,
    gap_threshold_seconds: &str,
    unmatched: &str,
    summary: &str,
) -> Result<String, JsValue> {
    let year_text = assume_year.trim();
    let year: i64 = if year_text.is_empty() {
        0
    } else {
        year_text.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "assumed year must be 0 (infer it from the paste) or a whole year between 1970 \
                 and 2100, got \"{year_text}\""
            ))
        })?
    };

    let gap_text = gap_threshold_seconds.trim();
    let gap: f64 = if gap_text.is_empty() {
        0.0
    } else {
        gap_text.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "gap threshold must be 0 (no gap marking) or a number of seconds, got \
                 \"{gap_text}\""
            ))
        })?
    };

    gizza_ai_log_timestamp_normalizer_core::run(
        log,
        or_default(output_format, "iso8601"),
        or_default(output_timezone, "UTC"),
        or_default(assume_timezone, "UTC"),
        year,
        or_default(sort, "input"),
        or_default(output_mode, "replace"),
        truthy(delta),
        or_default(delta_format, "auto"),
        gap,
        or_default(unmatched, "keep"),
        truthy(summary),
    )
    .map_err(|e| JsValue::from_str(&e))
}
