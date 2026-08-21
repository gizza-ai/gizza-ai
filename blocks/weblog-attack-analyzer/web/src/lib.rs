//! Browser-facing wasm-bindgen wrapper for /tools/weblog-attack-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    logs: &str,
    category: &str,
    min_severity: &str,
    output: &str,
    offender_threshold: &str,
    error_threshold: &str,
    decode: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let offender_threshold = offender_threshold.trim().parse::<u32>().unwrap_or(0);
    let error_threshold = error_threshold.trim().parse::<u32>().unwrap_or(0);
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    let decode = !matches!(
        decode.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    gizza_ai_weblog_attack_analyzer_core::analyze(
        logs,
        category,
        min_severity,
        output,
        offender_threshold,
        error_threshold,
        decode,
        limit,
    )
    .map_err(|e| JsValue::from_str(&e))
}
