//! Browser-facing wasm-bindgen wrapper for /tools/fastq-to-fasta/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the numeric/boolean/enum params are parsed here.
use gizza_ai_fastq_to_fasta_core::{convert, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Convert FASTQ text to FASTA.
///
/// - `fastq`: the FASTQ reads.
/// - `min_length`: minimum read length (blank/unparseable → 0 = no filter).
/// - `min_quality`: minimum mean Phred quality (blank/unparseable → 0 = no filter).
/// - `quality_offset`: `"33"` (default/blank) or `"64"`.
/// - `wrap`: sequence line width (blank/unparseable → 0 = one line; core caps at 1000).
/// - `rename` / `strip_n` / `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
///
/// Throws a JS error string on malformed FASTQ or an invalid `quality_offset`.
#[wasm_bindgen]
pub fn run(
    fastq: &str,
    min_length: &str,
    min_quality: &str,
    quality_offset: &str,
    wrap: &str,
    rename: &str,
    strip_n: &str,
    uppercase: &str,
) -> Result<String, JsValue> {
    let quality_offset = match quality_offset.trim() {
        "" | "33" => 33u8,
        "64" => 64u8,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid quality_offset {other:?}: expected \"33\" or \"64\""
            )))
        }
    };
    let opts = Options {
        min_length: min_length.trim().parse::<usize>().unwrap_or(0),
        min_quality: min_quality.trim().parse::<f64>().unwrap_or(0.0),
        quality_offset,
        wrap: wrap.trim().parse::<usize>().unwrap_or(0),
        rename: truthy(rename),
        strip_n: truthy(strip_n),
        uppercase: truthy(uppercase),
    };
    convert(fastq, &opts).map_err(|e| JsValue::from_str(&e))
}
