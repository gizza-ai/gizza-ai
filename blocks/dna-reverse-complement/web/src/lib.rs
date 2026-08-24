//! Browser-facing wasm-bindgen wrapper for /tools/dna-reverse-complement/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the numeric/boolean/enum params are parsed here.
use gizza_ai_dna_reverse_complement_core::{
    convert, parse_alphabet, parse_on_invalid, parse_operation, Options,
};
use wasm_bindgen::prelude::*;

/// Page checkboxes send "true"/"false"; be generous about the other truthy forms.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// A checkbox whose descriptor default is `true`: an empty value means "not
/// supplied" (deep link without the param), which must keep the default on.
fn truthy_default_on(v: &str) -> bool {
    if v.trim().is_empty() {
        true
    } else {
        truthy(v)
    }
}

/// Reverse-complement a DNA/RNA sequence.
///
/// - `sequence`: raw bases or FASTA.
/// - `operation`: `"reverse_complement"` (default/blank), `"complement"`, `"reverse"`.
/// - `output_alphabet`: `"auto"` (default/blank), `"dna"`, `"rna"`.
/// - `preserve_case`: blank → on (the descriptor default); `"false"` → uppercase output.
/// - `line_width`: wrap width (blank/unparseable → 0 = one line; core caps at 200).
/// - `on_invalid`: `"error"` (default/blank), `"drop"`, `"keep"`.
/// - `show_stats`: `"true"`/`"1"`/`"yes"`/`"on"` → append the composition summary.
///
/// Throws a JS error string on an invalid option value or a rejected sequence.
#[wasm_bindgen]
pub fn run(
    sequence: &str,
    operation: &str,
    output_alphabet: &str,
    preserve_case: &str,
    line_width: &str,
    on_invalid: &str,
    show_stats: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        operation: parse_operation(operation).map_err(|e| JsValue::from_str(&e))?,
        output_alphabet: parse_alphabet(output_alphabet).map_err(|e| JsValue::from_str(&e))?,
        preserve_case: truthy_default_on(preserve_case),
        line_width: line_width.trim().parse::<usize>().unwrap_or(0),
        on_invalid: parse_on_invalid(on_invalid).map_err(|e| JsValue::from_str(&e))?,
        show_stats: truthy(show_stats),
    };
    convert(sequence, &opts).map_err(|e| JsValue::from_str(&e))
}
