//! Browser-facing wasm-bindgen wrapper for /tools/set-diff-json/.
//! Field ORDER must match page/meta.toml: array_a, array_b, operation, key,
//! case_insensitive, dedupe, output, indent. The page hands every field over as
//! a string (checkboxes marshal as "true"/"false"), so the string → Options
//! coercion lives here and the set logic stays in the core.
use gizza_ai_set_diff_json_core::{set_diff, Operation, Options, Output};
use wasm_bindgen::prelude::*;

/// A checkbox arrives as "true"/"false"; a deep-link may omit it entirely, in
/// which case the descriptor default applies.
fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    array_a: &str,
    array_b: &str,
    operation: &str,
    key: &str,
    case_insensitive: &str,
    dedupe: &str,
    output: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent = match indent.trim() {
        "" => 2,
        s => s.parse::<usize>().map_err(|_| {
            JsValue::from_str(&format!("indent must be a whole number 0-8, got \"{s}\""))
        })?,
    };
    let opts = Options {
        operation: Operation::parse(operation).map_err(|e| JsValue::from_str(&e))?,
        key: key.trim().to_string(),
        case_insensitive: parse_bool(case_insensitive, false),
        dedupe: parse_bool(dedupe, true),
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
        indent: indent.min(8),
    };
    set_diff(array_a, array_b, &opts).map_err(|e| JsValue::from_str(&e))
}
