//! Browser-facing wasm-bindgen wrapper for /tools/data-clusterer/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the numeric options, builds the core Options, and returns the SVG /
//! CSV / JSON result.
use gizza_ai_data_clusterer_core::{run, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run_tool(
    data: &str,
    method: &str,
    clusters: &str,
    eps: &str,
    min_samples: &str,
    linkage: &str,
    columns: &str,
    normalize: &str,
    output: &str,
    title: &str,
    width: &str,
    height: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page shows a neutral idle state rather
    // than a red error on first load / after Reset).
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let opts = Options {
        method: if method.trim().is_empty() { "kmeans".to_string() } else { method.to_string() },
        clusters: clusters.trim().parse::<u32>().unwrap_or(3),
        eps: eps.trim().parse::<f64>().unwrap_or(1.0),
        min_samples: min_samples.trim().parse::<u32>().unwrap_or(4),
        linkage: if linkage.trim().is_empty() { "average".to_string() } else { linkage.to_string() },
        columns: columns.to_string(),
        // Page checkbox arrives as "true"/"false"; treat positive-truthy as on.
        normalize: matches!(normalize.trim(), "true" | "1" | "on" | "yes"),
        output: if output.trim().is_empty() { "chart".to_string() } else { output.to_string() },
        title: title.to_string(),
        width: width.trim().parse::<u32>().unwrap_or(700),
        height: height.trim().parse::<u32>().unwrap_or(500),
    };
    run(data, &opts).map_err(|e| JsValue::from_str(&e))
}
