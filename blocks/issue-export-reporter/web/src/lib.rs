//! Browser-facing wasm-bindgen wrapper for /tools/issue-export-reporter/.
//! Argument order MUST match meta.toml's `[[input]]` order: data, report,
//! format, delimiter, columns, done_statuses, cancelled_statuses, group_by,
//! period, unit, business_days, percentiles.
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as "true"/"false"; positive-truthy so an unchecked box
/// (and an untouched deep link) means calendar time.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Empty fields fall through to the core's documented defaults, so a blank box
/// behaves exactly like an omitted chat/CLI argument.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    report: &str,
    format: &str,
    delimiter: &str,
    columns: &str,
    done_statuses: &str,
    cancelled_statuses: &str,
    group_by: &str,
    period: &str,
    unit: &str,
    business_days: &str,
    percentiles: &str,
) -> Result<String, JsValue> {
    gizza_ai_issue_export_reporter_core::run(
        data,
        report,
        format,
        delimiter,
        columns,
        done_statuses,
        cancelled_statuses,
        group_by,
        period,
        unit,
        truthy(business_days),
        percentiles,
    )
    .map_err(|e| JsValue::from_str(&e))
}
