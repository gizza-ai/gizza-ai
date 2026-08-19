//! Browser-facing wasm-bindgen wrapper for /tools/api-response-diff/.
//! Field ORDER must match page/meta.toml: left, right, ignore, ignore_timestamps,
//! ignore_uuids, array_match, array_key, numeric_tolerance, ignore_case,
//! trim_strings, null_equals_missing, coerce_types, output, indent.
//! The page hands every field over as a string, so parsing lives in the core.
use gizza_ai_api_response_diff_core::{diff_responses, options_from_strings};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    ignore: &str,
    ignore_timestamps: &str,
    ignore_uuids: &str,
    array_match: &str,
    array_key: &str,
    numeric_tolerance: &str,
    ignore_case: &str,
    trim_strings: &str,
    null_equals_missing: &str,
    coerce_types: &str,
    output: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = options_from_strings(
        ignore,
        ignore_timestamps,
        ignore_uuids,
        array_match,
        array_key,
        numeric_tolerance,
        ignore_case,
        trim_strings,
        null_equals_missing,
        coerce_types,
        output,
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    diff_responses(left, right, &opts).map_err(|e| JsValue::from_str(&e))
}
