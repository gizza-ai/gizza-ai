//! Browser-facing wasm-bindgen wrapper for /tools/isolation-forest-anomaly/.
//! The standalone page passes every field value as a string, so the numeric and
//! boolean params arrive as strings and are parsed here. A blank or unparseable
//! field falls back to the core default rather than erroring, which keeps the
//! page's recompute-on-every-keystroke model usable mid-edit.
use gizza_ai_isolation_forest_anomaly_core::Options;
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Blank keeps the core default; anything else is passed through verbatim so
/// core owns the validation and the error message.
fn or_default(v: &str, fallback: String) -> String {
    if v.trim().is_empty() {
        fallback
    } else {
        v.trim().to_string()
    }
}

/// Fit an isolation forest over the numeric columns of `input` and report a
/// per-row anomaly score.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    features: &str,
    method: &str,
    trees: &str,
    sample_size: &str,
    max_features: &str,
    bootstrap: &str,
    contamination: &str,
    threshold: &str,
    missing: &str,
    sort: &str,
    top: &str,
    only_anomalies: &str,
    header: &str,
    delimiter: &str,
    decimals: &str,
    seed: &str,
    format: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let opts = Options {
        features: features.trim().to_string(),
        method: or_default(method, d.method),
        trees: trees.trim().parse().unwrap_or(d.trees),
        sample_size: or_default(sample_size, d.sample_size),
        max_features: max_features.trim().parse().unwrap_or(d.max_features),
        bootstrap: truthy(bootstrap),
        contamination: or_default(contamination, d.contamination),
        threshold: threshold.trim().parse().unwrap_or(d.threshold),
        missing: or_default(missing, d.missing),
        sort: or_default(sort, d.sort),
        top: top.trim().parse().unwrap_or(d.top),
        only_anomalies: truthy(only_anomalies),
        header: or_default(header, d.header),
        delimiter: or_default(delimiter, d.delimiter),
        decimals: decimals.trim().parse().unwrap_or(d.decimals),
        seed: seed.trim().parse().unwrap_or(d.seed),
        format: or_default(format, d.format),
    };
    gizza_ai_isolation_forest_anomaly_core::run(input, &opts).map_err(|e| JsValue::from_str(&e))
}
