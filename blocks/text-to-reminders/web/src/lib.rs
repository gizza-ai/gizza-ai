//! Browser-facing wasm-bindgen wrapper for /tools/text-to-reminders/.
//! Field order MUST match meta.toml: text, reference_date, detect_priority,
//! include_undated, alarm_minutes. When `reference_date` is blank the current
//! local date comes from the browser (`Date`), since wasm32-unknown-unknown has
//! no std clock.
use wasm_bindgen::prelude::*;

/// Page booleans arrive as "true"/"false" strings; treat positive-truthy as on.
fn truthy(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no" | ""
    )
}

/// The browser's current local date as `YYYY-MM-DD`.
fn today_local() -> String {
    let now = js_sys::Date::new_0();
    let y = now.get_full_year();
    let m = now.get_month() + 1; // JS months are 0-based
    let d = now.get_date();
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    reference_date: &str,
    detect_priority: &str,
    include_undated: &str,
    alarm_minutes: &str,
) -> Result<String, JsValue> {
    let reference = if reference_date.trim().is_empty() {
        today_local()
    } else {
        reference_date.trim().to_string()
    };
    let alarm = alarm_minutes.trim().parse::<i64>().unwrap_or(0);
    // Default the booleans to on when the field is absent/blank (matches the
    // descriptor `.default(true)`); an explicit "false" turns them off.
    let detect = if detect_priority.trim().is_empty() {
        true
    } else {
        truthy(detect_priority)
    };
    let undated = if include_undated.trim().is_empty() {
        true
    } else {
        truthy(include_undated)
    };
    gizza_ai_text_to_reminders_core::build_reminders(text, &reference, detect, undated, alarm)
        .map_err(|e| JsValue::from_str(&e))
}
