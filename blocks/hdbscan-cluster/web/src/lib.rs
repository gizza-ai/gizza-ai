//! Browser-facing wasm-bindgen wrapper for /tools/hdbscan-cluster/.
//! The standalone page passes every field value as a string, so the numeric and
//! boolean params arrive as strings and are parsed here. A blank or unparseable
//! field falls back to the core default rather than erroring, which keeps the
//! page's recompute-on-every-keystroke model usable mid-edit.
use gizza_ai_hdbscan_cluster_core::Options;
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

/// Cluster the numeric columns of `input` with HDBSCAN*.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    features: &str,
    min_cluster_size: &str,
    min_samples: &str,
    metric: &str,
    alpha: &str,
    cluster_selection_epsilon: &str,
    selection: &str,
    allow_single_cluster: &str,
    max_cluster_size: &str,
    normalize: &str,
    missing: &str,
    sort: &str,
    top: &str,
    only_noise: &str,
    header: &str,
    delimiter: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let opts = Options {
        features: features.trim().to_string(),
        min_cluster_size: min_cluster_size.trim().parse().unwrap_or(d.min_cluster_size),
        min_samples: min_samples.trim().parse().unwrap_or(d.min_samples),
        metric: or_default(metric, d.metric),
        alpha: alpha.trim().parse().unwrap_or(d.alpha),
        cluster_selection_epsilon: cluster_selection_epsilon
            .trim()
            .parse()
            .unwrap_or(d.cluster_selection_epsilon),
        selection: or_default(selection, d.selection),
        allow_single_cluster: truthy(allow_single_cluster),
        max_cluster_size: max_cluster_size.trim().parse().unwrap_or(d.max_cluster_size),
        normalize: truthy(normalize),
        missing: or_default(missing, d.missing),
        sort: or_default(sort, d.sort),
        top: top.trim().parse().unwrap_or(d.top),
        only_noise: truthy(only_noise),
        header: or_default(header, d.header),
        delimiter: or_default(delimiter, d.delimiter),
        decimals: decimals.trim().parse().unwrap_or(d.decimals),
        format: or_default(format, d.format),
    };
    gizza_ai_hdbscan_cluster_core::run(input, &opts).map_err(|e| JsValue::from_str(&e))
}
