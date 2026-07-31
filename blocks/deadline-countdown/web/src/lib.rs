//! Browser-facing wasm-bindgen wrapper for /tools/deadline-countdown/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    tasks: &str,
    now: &str,
    format: &str,
    include_completed: &str,
    soon_days: &str,
) -> Result<String, JsValue> {
    let soon_days = soon_days.trim().parse::<i64>().unwrap_or(7);
    gizza_ai_deadline_countdown_core::run(tasks, now, format, truthy(include_completed), soon_days)
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
