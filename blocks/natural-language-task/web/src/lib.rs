//! Browser-facing wasm-bindgen wrapper for /tools/natural-language-task/.
//! The page passes every field as a raw string (no coercion), so `run` parses
//! the booleans here and supplies the browser's local date when `reference_date`
//! is blank (wasm32-unknown-unknown has no std clock).
use gizza_ai_natural_language_task_core::to_todo_txt;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// The browser's local date as `YYYY-MM-DD` (page target has no std clock).
fn today_local() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1, // JS months are 0-based
        d.get_date(),
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    text: &str,
    reference_date: &str,
    add_creation_date: &str,
    detect_priority: &str,
    detect_due: &str,
    project: &str,
    context: &str,
) -> Result<String, JsValue> {
    let reference = if reference_date.trim().is_empty() {
        today_local()
    } else {
        reference_date.to_string()
    };
    to_todo_txt(
        text,
        &reference,
        truthy(add_creation_date),
        truthy(detect_priority),
        truthy(detect_due),
        project,
        context,
    )
    .map_err(|e| JsValue::from_str(&e))
}
