//! Browser-facing wasm-bindgen wrapper for /tools/faceted-filter/.
//! Compiled with wasm-pack for the standalone /tools/faceted-filter/ page.
use wasm_bindgen::prelude::*;

/// Run a faceted search over a JSON dataset.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `query` / `search_fields` / `filters` / `facets` / `sort`: passed through
///   (blank = no query / all values / no filtering / auto facets / input order).
/// - `facet_limit`: a non-negative integer (blank/unparseable → 10; 0 = unlimited).
/// - `facet_sort`: `"count_desc"` (blank → count_desc) / `"count_asc"` /
///   `"value_asc"` / `"value_desc"`.
/// - `facet_mode`: `"conjunctive"` (blank → conjunctive) / `"disjunctive"`.
/// - `facet_stats`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
/// - `page` / `per_page`: positive integers (blank/unparseable → 1 / 10).
/// - `output`: `"json"` (blank → json) / `"items"` / `"facets"` / `"summary"`.
///
/// Throws a JS error string on invalid JSON, a broken filter expression, an
/// unknown enum value, or out-of-range pagination.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    query: &str,
    search_fields: &str,
    filters: &str,
    facets: &str,
    facet_limit: &str,
    facet_sort: &str,
    facet_mode: &str,
    facet_stats: &str,
    sort: &str,
    page: &str,
    per_page: &str,
    output: &str,
) -> Result<String, JsValue> {
    let facet_limit = parse_usize(facet_limit, 10);
    let page = parse_usize(page, 1).max(1);
    let per_page = parse_usize(per_page, 10).max(1);
    let facet_stats = is_truthy(facet_stats);
    gizza_ai_faceted_filter_core::search(
        data,
        query,
        search_fields,
        filters,
        facets,
        facet_limit,
        facet_sort,
        facet_mode,
        facet_stats,
        sort,
        page,
        per_page,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_usize(v: &str, fallback: usize) -> usize {
    match v.trim() {
        "" => fallback,
        s => s.parse::<usize>().unwrap_or(fallback),
    }
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
