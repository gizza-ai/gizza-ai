//! Browser-facing wasm-bindgen wrapper for /tools/meeting-time-finder/.
//!
//! The tool page marshals every field as a string (checkboxes as "true"/"false"),
//! so the numeric and boolean params arrive as text and are parsed here; empty
//! fields fall back to the same defaults the chat schema declares.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, fallback: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "false" | "0" | "off" | "no" => false,
        _ => true,
    }
}

fn number(v: &str, fallback: f64, what: &str) -> Result<f64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{what} must be a number (got {t:?})"))
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    participants: &str,
    date: &str,
    duration_minutes: &str,
    granularity_minutes: &str,
    work_start: &str,
    work_end: &str,
    max_results: &str,
    clock: &str,
    display_zone: &str,
    skip_weekends: &str,
    allow_partial: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let duration = number(duration_minutes, 60.0, "duration_minutes").map_err(js)?;
    let wanted = number(max_results, 5.0, "max_results").map_err(js)?;
    gizza_ai_meeting_time_finder_core::find(
        participants,
        date,
        or_default(work_start, "09:00"),
        or_default(work_end, "17:00"),
        duration,
        or_default(granularity_minutes, "60"),
        wanted,
        or_default(clock, "24h"),
        display_zone,
        truthy(skip_weekends, true),
        truthy(allow_partial, true),
        or_default(output_format, "summary"),
    )
    .map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
