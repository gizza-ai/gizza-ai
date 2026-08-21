//! Browser-facing wasm-bindgen wrapper for /tools/tasks-to-ical/.
//! Field order MUST match meta.toml: tasks, component, include, skip_completed,
//! duration_minutes, reminder_minutes, timezone, calendar_name. Fields arrive as
//! strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Read a whole-number field, falling back to `fallback` when it is left empty.
fn minutes(value: &str, label: &str, fallback: i64) -> Result<i64, JsValue> {
    let text = value.trim();
    if text.is_empty() {
        return Ok(fallback);
    }
    text.parse().map_err(|_| {
        JsValue::from_str(&format!(
            "{label} must be a whole number of minutes, got \"{text}\""
        ))
    })
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    tasks: &str,
    component: &str,
    include: &str,
    skip_completed: &str,
    duration_minutes: &str,
    reminder_minutes: &str,
    timezone: &str,
    calendar_name: &str,
) -> Result<String, JsValue> {
    let duration = minutes(duration_minutes, "the event length", 60)?;
    let reminder = minutes(reminder_minutes, "the reminder lead time", 0)?;

    gizza_ai_tasks_to_ical_core::run(
        tasks,
        component,
        include,
        truthy(skip_completed),
        duration,
        reminder,
        timezone,
        calendar_name,
    )
    .map_err(|e| JsValue::from_str(&e))
}
