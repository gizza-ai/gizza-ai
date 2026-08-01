//! Browser-facing wasm-bindgen wrapper for /tools/genomic-vcf-to-tsv/.
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    layout: &str,
    include_info: &str,
    include_samples: &str,
    info_fields: &str,
    pass_only: &str,
    prefix_info: &str,
    missing: &str,
    header: &str,
) -> Result<String, JsValue> {
    let missing = if missing.trim().is_empty() {
        "."
    } else {
        missing
    };
    gizza_ai_genomic_vcf_to_tsv_core::run(
        input,
        layout,
        parse_bool(include_info, true),
        parse_bool(include_samples, true),
        info_fields,
        parse_bool(pass_only, false),
        parse_bool(prefix_info, false),
        missing,
        parse_bool(header, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
