//! Browser-facing wasm-bindgen wrapper for /tools/mongo-query/.
//! Compiled with wasm-pack for the standalone /tools/mongo-query/ page.
use wasm_bindgen::prelude::*;

/// Run a MongoDB-style query document against a pasted JSON array of documents.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `query` / `projection` / `sort`: passed through (blank = match all / whole
///   documents / input order).
/// - `skip` / `limit`: non-negative integers (blank or unparseable → 0).
/// - `format`: `"json"` (blank → json) / `"ndjson"` / `"csv"` / `"count"`.
/// - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else off.
///
/// Throws a JS error string on invalid documents, an invalid or unsupported
/// query, or a bad projection/sort/format.
#[wasm_bindgen]
pub fn run(
    data: &str,
    query: &str,
    projection: &str,
    sort: &str,
    skip: &str,
    limit: &str,
    format: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let skip = skip.trim().parse::<usize>().unwrap_or(0);
    let limit = limit.trim().parse::<usize>().unwrap_or(0);
    gizza_ai_mongo_query_core::run(data, query, projection, sort, skip, limit, format, is_truthy(pretty))
        .map_err(|e| JsValue::from_str(&e))
}

fn is_truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
