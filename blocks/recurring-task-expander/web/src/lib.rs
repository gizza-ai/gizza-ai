//! Browser-facing wasm-bindgen wrapper for /tools/recurring-task-expander/.
//! Compiled with wasm-pack for the standalone page. The wasm32-unknown-unknown
//! target has no std clock, so a blank `start` falls back to the browser's
//! current date (`Date.now()`).
use wasm_bindgen::prelude::*;

/// Expand recurring tasks into their next dated instances. The page passes each
/// field as a string, in the same order as the descriptor params: a blank
/// `start` means today, a blank `count` means 5, a blank `format` means "text",
/// and the checkbox arrives as "true"/"false". Throws a JS error string on
/// invalid input.
#[wasm_bindgen]
pub fn run(
    tasks: &str,
    start: &str,
    count: &str,
    default_rec: &str,
    skip_weekends: &str,
    format: &str,
) -> Result<String, JsValue> {
    let start_date = {
        let s = start.trim();
        if s.is_empty() {
            gizza_ai_recurring_task_expander_core::date_from_epoch_secs(
                (js_sys::Date::now() / 1000.0) as i64,
            )
        } else {
            s.to_string()
        }
    };
    let n: u32 = {
        let c = count.trim();
        if c.is_empty() {
            5
        } else {
            c.parse()
                .map_err(|_| JsValue::from_str("count must be a whole number between 1 and 100"))?
        }
    };
    let skip = matches!(
        skip_weekends.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_recurring_task_expander_core::expand(tasks, &start_date, n, default_rec, skip, format)
        .map_err(|e| JsValue::from_str(&e))
}
