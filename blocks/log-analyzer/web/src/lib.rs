//! Browser-facing wasm-bindgen wrapper for /tools/log-analyzer/.
//! Compiled with wasm-pack for the standalone /tools/log-analyzer/ page.
use wasm_bindgen::prelude::*;

/// Analyze raw `logs` into a summary / JSON report.
///
/// The standalone tool page passes every field value as a string, so the
/// integer `top` param arrives as a string and is parsed here:
/// - `format`: `auto` (blank) | `json` | `logfmt` | `syslog` | `common` | `combined`.
/// - `output`: `summary` (blank) | `json`.
/// - `top`:    a count 1–100 (blank/unparseable → 0 → the core default of 10).
/// - `bucket`: `auto` (blank) | `minute` | `hour` | `day`.
///
/// Throws a JS error string on an invalid `format`/`output`/`bucket` or
/// undetectable auto input.
#[wasm_bindgen]
pub fn run(logs: &str, format: &str, output: &str, top: &str, bucket: &str) -> Result<String, JsValue> {
    let top = top.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_log_analyzer_core::analyze(logs, format, output, top, bucket)
        .map_err(|e| JsValue::from_str(&e))
}
