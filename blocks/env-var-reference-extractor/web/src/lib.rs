//! Browser-facing wasm-bindgen wrapper for /tools/env-var-reference-extractor/.
//! Field order MUST match page/meta.toml: text, syntax, output, defined,
//! include_defined_in_source, skip_comments, ignore, only_undefined, sort.
//! The page passes every field as a string, so the booleans arrive as
//! "true"/"false" and are parsed here.
use gizza_ai_env_var_reference_extractor_core::extract;
use wasm_bindgen::prelude::*;

/// Page checkboxes marshal as "true"/"false"; accept the other positive forms too.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    syntax: &str,
    output: &str,
    defined: &str,
    include_defined_in_source: &str,
    skip_comments: &str,
    ignore: &str,
    only_undefined: &str,
    sort: &str,
) -> Result<String, JsValue> {
    extract(
        text,
        syntax,
        output,
        defined,
        truthy(include_defined_in_source),
        truthy(skip_comments),
        ignore,
        truthy(only_undefined),
        sort,
    )
    .map_err(|e| JsValue::from_str(&e))
}
