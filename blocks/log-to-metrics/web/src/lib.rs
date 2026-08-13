//! Browser-facing wasm-bindgen wrapper for /tools/log-to-metrics/.
//! Compiled with wasm-pack for the standalone page.
//!
//! The standalone tool page passes every field value as a string, so the one
//! numeric knob arrives as a string and the checkbox as "true"/"false"; each
//! falls back to the descriptor's default when the field is left blank:
//! - `data`: the pasted log (textarea).
//! - `format`: `"auto"` (default) / `"json"` / `"logfmt"` / `"csv"`.
//! - `group_by`: comma list of up to 5 field names, blank → one `(all)` row.
//! - `value_field`: numeric field to summarise, blank → counts only.
//! - `percentiles`: comma list, blank → `50,95,99`.
//! - `percentile_method`: `"linear"` (default) / `"nearest"`.
//! - `time_field`: blank → auto-detect, `none` → no rate column.
//! - `rate_unit`: `"auto"` (default) / `"second"` / `"minute"` / `"hour"`.
//! - `error_field` / `error_values`: blank → no error columns / built-in set.
//! - `limit`: 1–1000, default `20`.
//! - `other`: checkbox, default checked (`"true"`).
//! - `sort`: `"count"` (default) / `group` / `sum` / `avg` / `max` / `errors` / `p_top`.
//! - `output`: `"table"` (default) / `"json"` / `"csv"` / `"prometheus"`.
//! - `metric_prefix`: blank → `log`.
//!
//! Throws a JS error string on a non-numeric `limit`, an invalid enum value, an
//! out-of-range knob, empty or over-long input, or a log that parses to no
//! records at all.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    format: &str,
    group_by: &str,
    value_field: &str,
    percentiles: &str,
    percentile_method: &str,
    time_field: &str,
    rate_unit: &str,
    error_field: &str,
    error_values: &str,
    limit: &str,
    other: &str,
    sort: &str,
    output: &str,
    metric_prefix: &str,
) -> Result<String, JsValue> {
    let limit = parse_u32("limit", limit, 20).map_err(err)?;
    // Blank selects and blank text fields fall through to the core's own
    // "" → default handling.
    gizza_ai_log_to_metrics_core::aggregate(
        data,
        format,
        group_by,
        value_field,
        percentiles,
        percentile_method,
        time_field,
        rate_unit,
        error_field,
        error_values,
        limit,
        truthy(other),
        sort,
        output,
        metric_prefix,
    )
    .map_err(err)
}

fn err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

/// The checkbox sends "true"/"false"; anything else (including a blank field on
/// a surface that omits it) means unchecked.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// A blank field means "use the default"; anything unparseable is a hard error
/// rather than a silently substituted value.
fn parse_u32(name: &str, v: &str, default: u32) -> Result<u32, String> {
    match v.trim() {
        "" => Ok(default),
        s => s
            .parse::<u32>()
            .map_err(|_| format!("invalid {name} {s:?}: expected a whole number")),
    }
}
