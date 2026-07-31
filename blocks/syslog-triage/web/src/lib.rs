//! Browser-facing wasm-bindgen wrapper for /tools/syslog-triage/.
//! Compiled with wasm-pack for the standalone /tools/syslog-triage/ page.
use wasm_bindgen::prelude::*;

/// Parse + triage raw syslog / auth.log text.
///
/// The standalone tool page passes every field value as a string, so the
/// integer `limit` arrives as a string and is parsed here:
/// - `category`: `all` (blank) | `sudo` | `ssh` | `cron` | `session` | `account` | `other`.
/// - `only`:     `all` (blank) | `failed` | `success` — status filter.
/// - `output`:   `summary` (blank) | `table` | `json`.
/// - `limit`:    a count 1–5000 (blank/unparseable → 0 → the core default of 500).
///
/// Throws a JS error string on an invalid `category`/`only`/`output` or empty input.
#[wasm_bindgen]
pub fn run(
    logs: &str,
    category: &str,
    only: &str,
    output: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_syslog_triage_core::triage(logs, category, only, output, limit)
        .map_err(|e| JsValue::from_str(&e))
}
