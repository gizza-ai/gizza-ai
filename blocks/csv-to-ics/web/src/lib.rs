//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-ics/.
//! Field order MUST match meta.toml: csv, timezone, default_duration_minutes,
//! include_alarm. Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    csv: &str,
    timezone: &str,
    default_duration_minutes: &str,
    include_alarm: &str,
) -> Result<String, JsValue> {
    let minutes_text = default_duration_minutes.trim();
    let minutes: i64 = if minutes_text.is_empty() {
        60
    } else {
        minutes_text.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "default event length must be a whole number of minutes between 1 and 1440, got \"{minutes_text}\""
            ))
        })?
    };

    gizza_ai_csv_to_ics_core::run(
        csv,
        if timezone.trim().is_empty() {
            "floating"
        } else {
            timezone
        },
        minutes,
        truthy(include_alarm),
    )
    .map_err(|e| JsValue::from_str(&e))
}
