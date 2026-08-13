//! Browser-facing wasm-bindgen wrapper for /tools/lazy-load-attributer/.
//! Field order MUST match meta.toml: html, targets, decoding, skip_first,
//! eager_first, fetchpriority_first, respect_skip_markers, output. Every field
//! arrives as a string; checkboxes arrive as "true"/"false".
use gizza_ai_lazy_load_attributer_core::{run as core_run, Decoding, Options, Output, Targets};
use wasm_bindgen::prelude::*;

/// Positive-truthy: the page sends "true"/"false" for checkboxes.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    html: &str,
    targets: &str,
    decoding: &str,
    skip_first: &str,
    eager_first: &str,
    fetchpriority_first: &str,
    respect_skip_markers: &str,
    output: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let skip = skip_first.trim();
    let skip_first: usize = if skip.is_empty() {
        0
    } else {
        skip.parse()
            .map_err(|_| err(format!("skip_first must be a whole number (got '{skip}')")))?
    };
    let opts = Options {
        targets: Targets::parse(targets).map_err(err)?,
        decoding: Decoding::parse(decoding).map_err(err)?,
        skip_first,
        eager_first: truthy(eager_first),
        fetchpriority_first: truthy(fetchpriority_first),
        // An empty string means "field not sent" (deep link without the param),
        // which must keep the descriptor's default(true).
        respect_skip_markers: respect_skip_markers.trim().is_empty()
            || truthy(respect_skip_markers),
    };
    core_run(html, &opts, Output::parse(output).map_err(err)?).map_err(err)
}
