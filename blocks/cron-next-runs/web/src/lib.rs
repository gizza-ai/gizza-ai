//! Browser-facing wasm-bindgen wrapper for /tools/cron-next-runs/.
//! Compiled with wasm-pack for the standalone /tools/cron-next-runs/ page. The
//! wasm32-unknown-unknown target has no std clock, so when `after` is blank we
//! read the current time from JS (`Date.now()`).
use wasm_bindgen::prelude::*;

/// Compute the next run times for a cron expression. The page passes each field
/// as a string; a blank `after` falls back to the browser's current time, a
/// blank `count` falls back to 5, and a blank `format` to "text". Throws a JS
/// error string on invalid input.
#[wasm_bindgen]
pub fn run(expression: &str, after: &str, count: &str, format: &str) -> Result<String, JsValue> {
    let base = {
        let a = after.trim();
        if a.is_empty() {
            (js_sys::Date::now() / 1000.0) as i64
        } else {
            gizza_ai_cron_next_runs_core::parse_timestamp(a).map_err(|e| JsValue::from_str(&e))?
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
    gizza_ai_cron_next_runs_core::next_runs(expression, base, n, format)
        .map_err(|e| JsValue::from_str(&e))
}
