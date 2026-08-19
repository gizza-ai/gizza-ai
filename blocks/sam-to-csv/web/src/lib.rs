//! Browser-facing wasm-bindgen wrapper for /tools/sam-to-csv/.
//! The page driver passes every field as a raw string, so booleans/numbers are
//! parsed here and all validation stays in the shared core.
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    header: &str,
    flags: &str,
    tags: &str,
    tag_fields: &str,
    include_seq: &str,
    computed: &str,
    mapped_only: &str,
    primary_only: &str,
    min_mapq: &str,
    missing: &str,
) -> Result<String, JsValue> {
    let min_mapq = match min_mapq.trim() {
        "" => 0u32,
        n => n
            .parse::<u32>()
            .map_err(|_| JsValue::from_str(&format!("min_mapq '{n}' is not a whole number 0-255")))?,
    };
    let missing = if missing.is_empty() { "." } else { missing };
    gizza_ai_sam_to_csv_core::run(
        input,
        delimiter,
        parse_bool(header, true),
        flags,
        tags,
        tag_fields,
        parse_bool(include_seq, true),
        parse_bool(computed, false),
        parse_bool(mapped_only, false),
        parse_bool(primary_only, false),
        min_mapq,
        missing,
    )
    .map_err(|e| JsValue::from_str(&e))
}
