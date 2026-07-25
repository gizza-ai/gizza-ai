//! Browser-facing wasm-bindgen wrapper for /tools/critical-path-calculator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Run the Critical Path Method on a task list.
///
/// The standalone page passes every field value as a string:
/// - `tasks`: the task list, one task per line as `name, duration[, pred, ...]`.
/// - `format`: `report` (blank → report) or `json`.
///
/// Throws a JS error string on invalid input (cycle, unknown predecessor,
/// missing/negative duration, empty list, unknown format).
#[wasm_bindgen]
pub fn run(tasks: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_critical_path_calculator_core::analyze(tasks, format)
        .map_err(|e| JsValue::from_str(&e))
}
