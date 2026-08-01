//! Browser-facing wasm-bindgen wrapper for /tools/log-to-table/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: logs, preset, pattern, output, header, on_nomatch, limit.
use wasm_bindgen::prelude::*;

fn parse_bool(value: &str, default: bool) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        default
    } else {
        matches!(v.as_str(), "true" | "1" | "yes" | "on")
    }
}

fn parse_limit(value: &str) -> Result<u32, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        Ok(gizza_ai_log_to_table_core::DEFAULT_LIMIT)
    } else {
        v.parse::<u32>()
            .map_err(|_| JsValue::from_str("limit must be an integer from 1 to 5000"))
    }
}

/// Parse log lines into a Markdown table, CSV, TSV, or JSON.
#[wasm_bindgen]
pub fn run(
    logs: &str,
    preset: &str,
    pattern: &str,
    output: &str,
    header: &str,
    on_nomatch: &str,
    limit: &str,
) -> Result<String, JsValue> {
    gizza_ai_log_to_table_core::parse(
        logs,
        preset,
        pattern,
        output,
        parse_bool(header, true),
        on_nomatch,
        parse_limit(limit)?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
