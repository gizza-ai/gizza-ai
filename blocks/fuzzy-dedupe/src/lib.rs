//! gizza-ai/fuzzy-dedupe — remove near-duplicate rows/lines via fuzzy similarity.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fuzzy_dedupe_core::dedupe;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default = "default_true")]
    normalize_case: bool,
    #[serde(default = "default_true")]
    normalize_spacing: bool,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool { true }
fn default_threshold() -> f64 { 85.0 }
fn default_keep() -> String { "first".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The rows to de-duplicate: a CSV table, or a plain list with one value per line."))
        .param(Param::string("columns").default("").describe("Which columns to match on: comma-separated header names (needs header=true) or 1-based indices. Blank matches the whole row/line."))
        .param(Param::string("delimiter").default(",").describe("Field separator for CSV: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','; harmless for a newline list."))
        .param(Param::boolean("header").default(false).describe("Treat the first row as a header (re-emitted verbatim, and usable to name columns). Default false — leave off for a plain list of values."))
        .param(Param::integer("threshold").default(85).min(0.0).max(100.0).describe("Similarity cutoff 0–100 (edit-distance ratio). Rows at or above it are treated as the same. Higher = stricter (fewer merges). Default 85."))
        .param(Param::enumv("keep", ["first", "longest", "most_frequent"]).default("first").describe("Which row of each near-duplicate group survives: first (earliest), longest (most information), or most_frequent (the exact value seen most). Default first."))
        .param(Param::boolean("normalize_case").default(true).describe("Ignore letter case when comparing (so 'USA' and 'usa' match). Default true."))
        .param(Param::boolean("normalize_spacing").default(true).describe("Collapse and trim whitespace when comparing (so 'New  York' and 'New York' match). Default true."))
        .param(Param::enumv("output", ["deduped", "removed", "json"]).default("deduped").describe("Result: deduped (the cleaned data), removed (only the dropped near-duplicate rows), or json (structured groups + stats)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct FuzzyDedupe;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-dedupe",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove near-duplicate rows or lines by fuzzy similarity",
    skill(
        description = "Find and remove near-duplicate rows/lines that exact de-duplication misses — typos, casing, and spacing differences. Feed a CSV (pick `columns` by header name or 1-based index) or a plain newline list (leave `columns` blank). Rows are grouped by a normalized Levenshtein similarity: two rows merge when their similarity (0–100) is at least `threshold` (default 85). `normalize_case`/`normalize_spacing` fold case and whitespace before comparing. `keep` chooses each group's survivor: first, longest, or most_frequent. `output` is deduped (the cleaned data), removed (only the dropped rows), or json (groups + stats). Runs locally.",
        parameters = schema_json()
    ),
)]
impl FuzzyDedupe {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fuzzy-dedupe", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let keep = if a.keep.is_empty() { "first".to_string() } else { a.keep };
            let output = if a.output.is_empty() { "deduped".to_string() } else { a.output };
            dedupe(
                &a.data,
                &a.columns,
                &delim,
                a.header,
                a.threshold,
                &keep,
                a.normalize_case,
                a.normalize_spacing,
                &output,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":              { "type": "string", "description": "The rows to de-duplicate: a CSV table, or a plain list with one value per line." },
                    "columns":           { "type": "string", "default": "", "description": "Which columns to match on: comma-separated header names (needs header=true) or 1-based indices. Blank matches the whole row/line." },
                    "delimiter":         { "type": "string", "default": ",", "description": "Field separator for CSV: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','; harmless for a newline list." },
                    "header":            { "type": "boolean", "default": false, "description": "Treat the first row as a header (re-emitted verbatim, and usable to name columns). Default false — leave off for a plain list of values." },
                    "threshold":         { "type": "integer", "minimum": 0, "maximum": 100, "default": 85, "description": "Similarity cutoff 0–100 (edit-distance ratio). Rows at or above it are treated as the same. Higher = stricter (fewer merges). Default 85." },
                    "keep":              { "type": "string", "enum": ["first", "longest", "most_frequent"], "default": "first", "description": "Which row of each near-duplicate group survives: first (earliest), longest (most information), or most_frequent (the exact value seen most). Default first." },
                    "normalize_case":    { "type": "boolean", "default": true, "description": "Ignore letter case when comparing (so 'USA' and 'usa' match). Default true." },
                    "normalize_spacing": { "type": "boolean", "default": true, "description": "Collapse and trim whitespace when comparing (so 'New  York' and 'New York' match). Default true." },
                    "output":            { "type": "string", "enum": ["deduped", "removed", "json"], "default": "deduped", "description": "Result: deduped (the cleaned data), removed (only the dropped near-duplicate rows), or json (structured groups + stats)." }
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
