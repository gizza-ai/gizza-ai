//! Browser-facing wasm-bindgen wrapper for /tools/regex-codemod/.
//! Parameter ORDER must match the `[[input]]` order in page/meta.toml.
use gizza_ai_regex_codemod_core::{run, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run_codemod(
    text: &str,
    pattern: &str,
    replacement: &str,
    literal: &str,
    ignore_case: &str,
    multiline: &str,
    dot_matches_newline: &str,
    whole_word: &str,
    replace_all: &str,
    file_marker: &str,
    marker_regex: &str,
    output: &str,
    context: &str,
    include_unchanged: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        literal: truthy(literal),
        ignore_case: truthy(ignore_case),
        multiline: truthy(multiline),
        dot_matches_newline: truthy(dot_matches_newline),
        whole_word: truthy(whole_word),
        replace_all: truthy(replace_all),
        context: context.trim().parse::<usize>().unwrap_or(3),
        include_unchanged: truthy(include_unchanged),
    };
    let file_marker = if file_marker.trim().is_empty() {
        "auto"
    } else {
        file_marker
    };
    let output = if output.trim().is_empty() {
        "diff"
    } else {
        output
    };
    run(
        text,
        pattern,
        replacement,
        file_marker,
        marker_regex,
        output,
        &opts,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
