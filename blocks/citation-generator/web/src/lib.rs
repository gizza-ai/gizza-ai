//! Browser-facing wasm-bindgen wrapper for /tools/citation-generator/.
//! Compiled with wasm-pack for the standalone /tools/citation-generator/ page.
use gizza_ai_citation_generator_core::{cite, Fields};
use wasm_bindgen::prelude::*;

/// Format a reference into APA/MLA/Chicago style. The standalone tool page
/// passes every field as a string (blank when omitted); the field ORDER here
/// must match `page/meta.toml`'s `[[input]]` order.
///
/// Throws a JS error string on an unknown style or when there is nothing to
/// cite (no title and no author).
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    style: &str,
    title: &str,
    authors: &str,
    year: &str,
    container: &str,
    publisher: &str,
    volume: &str,
    issue: &str,
    pages: &str,
    url: &str,
    doi: &str,
    accessed: &str,
) -> Result<String, JsValue> {
    let fields = Fields {
        authors: authors.to_string(),
        title: title.to_string(),
        year: year.to_string(),
        container: container.to_string(),
        publisher: publisher.to_string(),
        volume: volume.to_string(),
        issue: issue.to_string(),
        pages: pages.to_string(),
        url: url.to_string(),
        doi: doi.to_string(),
        accessed: accessed.to_string(),
    };
    cite(style, &fields).map_err(|e| JsValue::from_str(&e))
}
