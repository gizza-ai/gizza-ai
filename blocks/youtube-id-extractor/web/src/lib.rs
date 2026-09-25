//! Browser-facing wasm-bindgen wrapper for /tools/youtube-id-extractor/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    urls: &str,
    format: &str,
    timestamp: &str,
    thumbnail: &str,
    canonical: &str,
    embed: &str,
    strict: &str,
) -> Result<String, JsValue> {
    gizza_ai_youtube_id_extractor_core::extract(
        urls,
        format,
        timestamp,
        thumbnail,
        truthy(canonical),
        truthy(embed),
        truthy(strict),
    )
    .map_err(|e| JsValue::from_str(&e))
}
