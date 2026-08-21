//! Browser-facing wasm-bindgen wrapper for /tools/k8s-manifest-splitter/.
//! Field order MUST match meta.toml: manifest, output, filename_template, include,
//! exclude, sort, skip_non_k8s, expand_lists, include_triple_dash.
use gizza_ai_k8s_manifest_splitter_core::{split, Options, Output, Sort, DEFAULT_TEMPLATE};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    manifest: &str,
    output: &str,
    filename_template: &str,
    include: &str,
    exclude: &str,
    sort: &str,
    skip_non_k8s: &str,
    expand_lists: &str,
    include_triple_dash: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
        filename_template: if filename_template.trim().is_empty() {
            DEFAULT_TEMPLATE.to_string()
        } else {
            filename_template.to_string()
        },
        include: include.to_string(),
        exclude: exclude.to_string(),
        sort: Sort::parse(sort).map_err(|e| JsValue::from_str(&e))?,
        skip_non_k8s: truthy(skip_non_k8s),
        expand_lists: truthy(expand_lists),
        include_triple_dash: truthy(include_triple_dash),
    };
    split(manifest, &opts).map_err(|e| JsValue::from_str(&e))
}
