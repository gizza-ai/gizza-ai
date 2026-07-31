//! Browser-facing wasm-bindgen wrapper for /tools/correlated-feature-pruner/.
//! Compiled with wasm-pack for the standalone /tools/correlated-feature-pruner/ page.
use wasm_bindgen::prelude::*;

/// Positive-truthy checkbox parse (the page sends "true"/"false"); empty falls
/// back to `default`.
fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

/// Prune collinear numeric columns from `data`.
///
/// The standalone tool page passes every field value as a string, so:
/// - `threshold`: absolute-correlation cutoff in 0..1; blank or `0` → 0.9 default.
/// - `method`: `"pearson"` (default) / `"spearman"` / `"kendall"`.
/// - `labels`: optional comma-separated column names (blank → v1..vN / header).
/// - `header`: `"true"`/`"false"` — treat the first row as column names.
///
/// Throws a JS error string on a non-numeric threshold, an invalid method, or a
/// malformed table.
#[wasm_bindgen]
pub fn run(
    data: &str,
    threshold: &str,
    method: &str,
    labels: &str,
    header: &str,
) -> Result<String, JsValue> {
    let threshold = {
        let t = threshold.trim();
        if t.is_empty() {
            0.9
        } else {
            let v = t
                .parse::<f64>()
                .map_err(|_| JsValue::from_str("threshold must be a number between 0 and 1"))?;
            // Mirror the chat/CLI handler: 0 is a degenerate all-prune setting, so
            // treat it as the documented 0.9 default.
            if v == 0.0 {
                0.9
            } else {
                v
            }
        }
    };
    let method = if method.trim().is_empty() {
        "pearson"
    } else {
        method
    };
    gizza_ai_correlated_feature_pruner_core::prune_report(
        data,
        threshold,
        method,
        labels,
        truthy(header, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
