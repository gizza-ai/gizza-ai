//! Browser-facing wasm-bindgen wrapper for /tools/search-index-builder/.
//! Compiled with wasm-pack for the standalone /tools/search-index-builder/ page.
use wasm_bindgen::prelude::*;

/// Build a serialized inverted-index JSON from a pasted JSON array of documents.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `lowercase`/`remove_stopwords`/`pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
///   `lowercase` defaults ON when blank (matches the schema default `true`);
///   the other two default OFF when blank.
/// - `min_length`: a count `1`–20 (blank/unparseable → 1; the core clamps 1..=20).
///
/// Throws a JS error string when `documents` is not a JSON array of objects,
/// on a duplicate ref, or on a malformed boost spec.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    documents: &str,
    fields: &str,
    id_field: &str,
    store_fields: &str,
    boosts: &str,
    lowercase: &str,
    remove_stopwords: &str,
    min_length: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    // `lowercase` is a default-ON checkbox: blank means "checked" isn't sent, but
    // the page renders it checked from the schema default, so blank → on here too.
    let lowercase = matches!(
        lowercase.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "yes" | "on"
    );
    let remove_stopwords = matches!(
        remove_stopwords.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let min_length = min_length.trim().parse::<usize>().unwrap_or(1);
    gizza_ai_search_index_builder_core::build_index(
        documents,
        fields,
        id_field,
        store_fields,
        boosts,
        lowercase,
        remove_stopwords,
        min_length,
        pretty,
    )
    .map_err(|e| JsValue::from_str(&e))
}
