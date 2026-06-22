//! Browser-facing wasm-bindgen wrapper for /tools/utm-link-builder/.
//! The standalone tool page passes every field value as a string. Returns the
//! full tagged URL (the most useful single text output for the page).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    url: &str,
    source: &str,
    medium: &str,
    campaign: &str,
    term: &str,
    content: &str,
    id: &str,
    source_platform: &str,
    creative_format: &str,
    marketing_tactic: &str,
    lowercase: &str,
) -> Result<String, JsValue> {
    let lowercase = matches!(
        lowercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let built = gizza_ai_utm_link_builder_core::build(
        &gizza_ai_utm_link_builder_core::Utm {
            url,
            source,
            medium,
            campaign,
            term,
            content,
            id,
            source_platform,
            creative_format,
            marketing_tactic,
        },
        lowercase,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(built.url)
}
