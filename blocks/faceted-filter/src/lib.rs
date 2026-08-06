//! gizza-ai/faceted-filter — faceted search over a JSON dataset, as a chat skill
//! block on the shared tool abstraction.
//!
//! Thin wrapper around `gizza-ai-faceted-filter-core`. The chat schema is derived
//! from `descriptor()` (single source — shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    search_fields: String,
    #[serde(default)]
    filters: String,
    #[serde(default)]
    facets: String,
    #[serde(default = "default_facet_limit")]
    facet_limit: u32,
    #[serde(default)]
    facet_sort: String,
    #[serde(default)]
    facet_mode: String,
    #[serde(default)]
    facet_stats: bool,
    #[serde(default)]
    sort: String,
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_per_page")]
    per_page: u32,
    #[serde(default)]
    output: String,
}

fn default_facet_limit() -> u32 {
    10
}
fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    10
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The dataset: a JSON array of records, e.g. [{\"brand\":\"Apple\",\"price\":999,\"tags\":[\"phone\"]}]. A single JSON object is treated as a one-record dataset, and NDJSON / JSON Lines (one JSON value per line) is accepted too. Maximum 50000 records."),
        )
        .param(
            Param::string("query")
                .default("")
                .describe("Free-text search; blank matches every record. Terms are split on whitespace and ALL of them must appear (case-insensitive substring) somewhere in the record's string, number or boolean values. Example: wireless black."),
        )
        .param(
            Param::string("search_fields")
                .default("")
                .describe("Comma-separated dotted paths the free-text query searches; blank searches every value in the record. Example: name, meta.summary."),
        )
        .param(
            Param::string("filters")
                .default("")
                .describe("Filter expression evaluated per record; blank keeps every record. A clause is 'path op value' (e.g. price < 500, brand == Apple, tags contains sale, name ~ ^A, brand in [Apple, Sony]). Ops: == != > >= < <= contains startswith endswith ~/matches in. Join clauses with and / or / not and group with ( ). A bare path (e.g. in_stock) is true when that field exists and is truthy. Dotted paths (meta.color), array indexes (items.0.id) and array fields (a clause holds if ANY element matches) are supported."),
        )
        .param(
            Param::string("facets")
                .default("")
                .describe("Comma-separated dotted paths to count values for, e.g. brand, tags, meta.color. Blank auto-detects the first 20 top-level fields holding scalars or arrays of scalars. Array elements are counted individually, so a facet's counts can exceed the match total."),
        )
        .param(
            Param::integer("facet_limit")
                .default(10)
                .min(0.0)
                .max(1000.0)
                .describe("Maximum values reported per facet (default 10); 0 = unlimited. Each facet also reports 'distinct', the full number of values found, so truncation is visible."),
        )
        .param(
            Param::enumv(
                "facet_sort",
                ["count_desc", "count_asc", "value_asc", "value_desc"],
            )
            .default("count_desc")
            .describe("Order of the values inside each facet. 'count_desc' (default) is most-common first, ties broken by value; 'count_asc' is rarest first; 'value_asc'/'value_desc' order by the value itself (numerically when the values are numbers)."),
        )
        .param(
            Param::enumv("facet_mode", ["conjunctive", "disjunctive"])
                .default("conjunctive")
                .describe("How facet counts relate to 'filters'. 'conjunctive' (default) counts only the records shown, so counts match the result list. 'disjunctive' ignores a facet's OWN filter clauses when counting that facet, so the other values of a facet the user already filtered on keep non-zero counts — the behavior multi-select facet sidebars expect. Both modes return the same filtered records."),
        )
        .param(
            Param::boolean("facet_stats")
                .default(false)
                .describe("When true, each facet whose values include numbers also reports stats: count, min, max, avg and sum over the records in scope. Default false."),
        )
        .param(
            Param::string("sort")
                .default("")
                .describe("Sort order for the returned records; blank keeps the input order. Comma-separated 'path' or 'path:desc' entries (a leading '-' also means descending), e.g. price:desc, name. Missing and null values sort last; ties keep input order."),
        )
        .param(
            Param::integer("page")
                .default(1)
                .min(1.0)
                .describe("1-based page number of records to return (default 1). A page past the end returns an empty 'items' list with the real total, not an error."),
        )
        .param(
            Param::integer("per_page")
                .default(10)
                .min(1.0)
                .max(1000.0)
                .describe("Records returned per page (default 10, maximum 1000). Facet counts always cover the whole match set, not just this page."),
        )
        .param(
            Param::enumv("output", ["json", "items", "facets", "summary"])
                .default("json")
                .describe("What to return. 'json' (default): the full envelope { total, page, per_page, total_pages, items, facets }. 'items': just this page's records. 'facets': just the facet breakdown. 'summary': a plain-text report of the totals and each facet's values."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FacetedFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/faceted-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Faceted search over a JSON dataset: filtered records plus facet value counts, sorting and pagination.",
    skill(
        description = "Run a faceted search over a JSON dataset (an array of records). 'query' is a free-text search across the records' values and 'filters' is a per-record expression of 'path op value' clauses (ops == != > >= < <= contains startswith endswith ~/matches in) joined by and / or / not with ( ) grouping; a bare path means exists-and-truthy, dotted paths and array fields work. Matching records are sorted by 'sort' ('price:desc, name') and paginated with 'page'/'per_page'. Every field in 'facets' (blank auto-detects up to 20) is broken down into value → count buckets computed from the current result set, capped by 'facet_limit' and ordered by 'facet_sort'; 'facet_mode=disjunctive' ignores a facet's own filter clauses when counting it (multi-select facet behavior) and 'facet_stats' adds min/max/avg/sum for numeric facets. 'output' selects the full JSON envelope { total, page, per_page, total_pages, items, facets }, just the items, just the facets, or a plain-text summary.",
        parameters = schema_json()
    ),
)]
impl FacetedFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "faceted-filter", |a: Args| {
            gizza_ai_faceted_filter_core::search(
                &a.data,
                &a.query,
                &a.search_fields,
                &a.filters,
                &a.facets,
                a.facet_limit as usize,
                &a.facet_sort,
                &a.facet_mode,
                a.facet_stats,
                &a.sort,
                a.page as usize,
                a.per_page as usize,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The dataset: a JSON array of records, e.g. [{\"brand\":\"Apple\",\"price\":999,\"tags\":[\"phone\"]}]. A single JSON object is treated as a one-record dataset, and NDJSON / JSON Lines (one JSON value per line) is accepted too. Maximum 50000 records." },
                    "query": { "type": "string", "default": "", "description": "Free-text search; blank matches every record. Terms are split on whitespace and ALL of them must appear (case-insensitive substring) somewhere in the record's string, number or boolean values. Example: wireless black." },
                    "search_fields": { "type": "string", "default": "", "description": "Comma-separated dotted paths the free-text query searches; blank searches every value in the record. Example: name, meta.summary." },
                    "filters": { "type": "string", "default": "", "description": "Filter expression evaluated per record; blank keeps every record. A clause is 'path op value' (e.g. price < 500, brand == Apple, tags contains sale, name ~ ^A, brand in [Apple, Sony]). Ops: == != > >= < <= contains startswith endswith ~/matches in. Join clauses with and / or / not and group with ( ). A bare path (e.g. in_stock) is true when that field exists and is truthy. Dotted paths (meta.color), array indexes (items.0.id) and array fields (a clause holds if ANY element matches) are supported." },
                    "facets": { "type": "string", "default": "", "description": "Comma-separated dotted paths to count values for, e.g. brand, tags, meta.color. Blank auto-detects the first 20 top-level fields holding scalars or arrays of scalars. Array elements are counted individually, so a facet's counts can exceed the match total." },
                    "facet_limit": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 10, "description": "Maximum values reported per facet (default 10); 0 = unlimited. Each facet also reports 'distinct', the full number of values found, so truncation is visible." },
                    "facet_sort": { "type": "string", "enum": ["count_desc", "count_asc", "value_asc", "value_desc"], "default": "count_desc", "description": "Order of the values inside each facet. 'count_desc' (default) is most-common first, ties broken by value; 'count_asc' is rarest first; 'value_asc'/'value_desc' order by the value itself (numerically when the values are numbers)." },
                    "facet_mode": { "type": "string", "enum": ["conjunctive", "disjunctive"], "default": "conjunctive", "description": "How facet counts relate to 'filters'. 'conjunctive' (default) counts only the records shown, so counts match the result list. 'disjunctive' ignores a facet's OWN filter clauses when counting that facet, so the other values of a facet the user already filtered on keep non-zero counts — the behavior multi-select facet sidebars expect. Both modes return the same filtered records." },
                    "facet_stats": { "type": "boolean", "default": false, "description": "When true, each facet whose values include numbers also reports stats: count, min, max, avg and sum over the records in scope. Default false." },
                    "sort": { "type": "string", "default": "", "description": "Sort order for the returned records; blank keeps the input order. Comma-separated 'path' or 'path:desc' entries (a leading '-' also means descending), e.g. price:desc, name. Missing and null values sort last; ties keep input order." },
                    "page": { "type": "integer", "minimum": 1, "default": 1, "description": "1-based page number of records to return (default 1). A page past the end returns an empty 'items' list with the real total, not an error." },
                    "per_page": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 10, "description": "Records returned per page (default 10, maximum 1000). Facet counts always cover the whole match set, not just this page." },
                    "output": { "type": "string", "enum": ["json", "items", "facets", "summary"], "default": "json", "description": "What to return. 'json' (default): the full envelope { total, page, per_page, total_pages, items, facets }. 'items': just this page's records. 'facets': just the facet breakdown. 'summary': a plain-text report of the totals and each facet's values." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
