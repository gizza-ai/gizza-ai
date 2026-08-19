//! Browser-facing wasm-bindgen wrapper for /tools/super-timeline-builder/.
//! Field order MUST match meta.toml: artifacts, format, order, expand, dedupe,
//! from, to, tz_offset, drop_epoch_zero, delimiter, limit.
//! Fields arrive as strings (checkboxes send "true"/"false"); numerics are f64
//! so wasm-bindgen never hands JS a BigInt.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Empty enum/number fields fall back to the schema default.
fn or(s: &str, fallback: &'static str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    artifacts: &str,
    format: &str,
    order: &str,
    expand: &str,
    dedupe: &str,
    from: &str,
    to: &str,
    tz_offset: f64,
    drop_epoch_zero: &str,
    delimiter: &str,
    limit: f64,
) -> Result<String, JsValue> {
    let limit = if limit <= 0.0 { 10_000.0 } else { limit };
    gizza_ai_super_timeline_builder_core::build(
        artifacts,
        &or(format, "csv"),
        &or(order, "asc"),
        truthy(expand),
        truthy(dedupe),
        from,
        to,
        tz_offset,
        truthy(drop_epoch_zero),
        &or(delimiter, "auto"),
        limit.round() as u32,
    )
    .map_err(|e| JsValue::from_str(&e))
}
