//! Browser-facing wasm-bindgen wrapper for /tools/ics-agenda-view/.
//! Argument order MUST match meta.toml's `[[input]]` order.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() {
        default
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    ics: &str,
    start_date: &str,
    days: i64,
    timezone: &str,
    day_start: &str,
    day_end: &str,
    min_gap_minutes: i64,
    show_gaps: &str,
    filter: &str,
    expand_recurring: &str,
    include_cancelled: &str,
    details: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_ics_agenda_view_core::run(
        ics,
        start_date,
        days,
        timezone,
        day_start,
        day_end,
        min_gap_minutes,
        truthy(show_gaps, true),
        filter,
        truthy(expand_recurring, true),
        truthy(include_cancelled, false),
        details,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
