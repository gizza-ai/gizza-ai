//! Browser-facing wasm-bindgen wrapper for /tools/log-parser/.
//! Compiled with wasm-pack for the standalone /tools/log-parser/ page.
use wasm_bindgen::prelude::*;

/// Parse raw `logs` into a structured table / JSON / CSV.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `format`: `auto` (blank) | `json` | `logfmt` | `syslog` | `common` | `combined`.
/// - `output`: `table` (blank) | `json` | `csv`.
/// - `level`:  `all` (blank) | `error` | `warn` | `info` | `debug` | `trace`.
/// - `filter`: text to keep matching lines (substring, or regex when `regex` is on).
/// - `regex`:  `"true"`/`"1"`/`"yes"`/`"on"` → treat `filter` as a regex; else off.
/// - `limit`:  a count 1–5000 (blank/unparseable → 0 → the core default of 200).
///
/// Throws a JS error string on an invalid `format`/`output`/`level`, an invalid
/// regex, or undetectable auto input.
#[wasm_bindgen]
pub fn run(
    logs: &str,
    format: &str,
    output: &str,
    level: &str,
    filter: &str,
    regex: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let use_regex = matches!(
        regex.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_log_parser_core::parse(logs, format, output, level, filter, use_regex, limit)
        .map_err(|e| JsValue::from_str(&e))
}
