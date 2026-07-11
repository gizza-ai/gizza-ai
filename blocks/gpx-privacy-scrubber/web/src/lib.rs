//! Browser-facing wasm-bindgen wrapper for /tools/gpx-privacy-scrubber/.
//! Compiled with wasm-pack for the standalone /tools/gpx-privacy-scrubber/ page.
use wasm_bindgen::prelude::*;

/// Positive-truthy parse of a page checkbox value (`"true"`/`"1"`/`"yes"`/`"on"`).
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Scrub a GPX file. The standalone tool page passes every field value as a
/// string; `radius_m` arrives as text (default 200 when blank/invalid) and the
/// two checkboxes arrive as `"true"`/`"false"`.
///
/// Throws a JS error string on empty input or a document with no GPX points.
#[wasm_bindgen]
pub fn run(
    gpx: &str,
    mode: &str,
    radius_m: &str,
    scrub_timestamps: &str,
    remove_extensions: &str,
) -> Result<String, JsValue> {
    let radius_m = radius_m.trim().parse::<u32>().unwrap_or(200);
    gizza_ai_gpx_privacy_scrubber_core::run(
        gpx,
        mode,
        radius_m,
        truthy(scrub_timestamps),
        truthy(remove_extensions),
    )
    .map_err(|e| JsValue::from_str(&e))
}
