//! Browser-facing wasm-bindgen wrapper for /tools/fake-data-generator/.
//! The standalone tool page passes every field value as a string, so the
//! numeric params arrive as strings and are parsed here.
use wasm_bindgen::prelude::*;

/// Generate fake records.
///
/// - `count`: rows, 1–1000 (blank/unparseable → 10; the core enforces bounds).
/// - `format`: `"csv"` / `"json"` (blank → csv).
/// - `fields`: comma-separated column subset, `"all"`, or blank for defaults.
/// - `seed`: reproducibility seed; blank or `0` derives a fresh time-based seed
///   so each click varies, while any non-zero value reproduces the dataset.
#[wasm_bindgen]
pub fn run(
    count: &str,
    format: &str,
    fields: &str,
    seed: &str,
    header: &str,
    table: &str,
) -> Result<String, JsValue> {
    let count = count.trim().parse::<u32>().unwrap_or(10);
    let parsed_seed = seed.trim().parse::<u64>().unwrap_or(0);
    let seed = if parsed_seed == 0 {
        // No std clock on wasm32-unknown-unknown — use JS Date for entropy.
        (js_sys::Date::now() as u64).max(1)
    } else {
        parsed_seed
    };
    // A default-true checkbox sends "true" when checked, "false" when not.
    let header = matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_fake_data_generator_core::generate_opts(count, format, fields, seed, header, table)
        .map_err(|e| JsValue::from_str(&e))
}
