//! Browser-facing wasm-bindgen wrapper for /tools/focus-picker/.
use wasm_bindgen::prelude::*;

fn flag(value: &str, default: bool) -> bool {
    let t = value.trim();
    if t.is_empty() {
        default
    } else {
        matches!(t, "true" | "1" | "on" | "yes")
    }
}

fn num(value: f64, default: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        default
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    tasks: &str,
    method: &str,
    today: &str,
    default_priority: &str,
    default_effort: f64,
    overdue_boost: &str,
    format: &str,
    show_ranking: &str,
) -> Result<String, JsValue> {
    let today_days = gizza_ai_focus_picker_core::resolve_today(today, js_sys::Date::now() / 1000.0)
        .map_err(|e| JsValue::from_str(&e))?;
    let opts = gizza_ai_focus_picker_core::Options {
        tasks,
        method,
        today_days,
        default_priority,
        default_effort: num(default_effort, 2.0),
        overdue_boost: flag(overdue_boost, true),
        format,
        show_ranking: flag(show_ranking, true),
    };
    gizza_ai_focus_picker_core::pick(&opts).map_err(|e| JsValue::from_str(&e))
}
