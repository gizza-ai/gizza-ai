//! gizza-ai/cluster-similar-values — fuzzy-cluster near-duplicate text values.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cluster_similar_values_core::cluster;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    column: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_true")]
    normalize_case: bool,
    #[serde(default = "default_true")]
    normalize_spacing: bool,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool { true }
fn default_threshold() -> f64 { 85.0 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The values to cluster: a CSV table, or a plain list with one value per line."))
        .param(Param::string("column").default("").describe("Which column to cluster: a header name (needs header=true) or a 1-based index. Blank uses the only column (e.g. a newline list)."))
        .param(Param::string("delimiter").default(",").describe("Field separator for CSV: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','; harmless for a newline list."))
        .param(Param::boolean("header").default(false).describe("Treat the first row as a header (skipped from clustering, and usable to name the column). Default false — leave off for a plain list of values."))
        .param(Param::integer("threshold").default(85).min(0.0).max(100.0).describe("Similarity cutoff 0–100 (edit-distance ratio). Values at or above it join the same cluster. Higher = stricter/tighter clusters. Default 85."))
        .param(Param::boolean("normalize_case").default(true).describe("Ignore letter case when comparing (so 'USA' and 'usa' match). Default true."))
        .param(Param::boolean("normalize_spacing").default(true).describe("Collapse and trim whitespace when comparing (so 'New  York' and 'New York' match). Default true."))
        .param(Param::enumv("output", ["markdown", "csv", "json"]).default("markdown").describe("Result format: markdown (clusters + a mapping table), csv (a flat original→canonical mapping), or json (structured clusters)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct ClusterSimilarValues;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cluster-similar-values",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fuzzy-cluster near-duplicate text values in a column",
    skill(
        description = "Fuzzy-cluster near-duplicate text values in one column and propose a canonical form per cluster. Feed a CSV (pick the `column` by header name or 1-based index) or a plain newline list (leave `column` blank). Values that differ only by typos, casing, or spacing are grouped by a normalized Levenshtein similarity: two values join when their similarity (0–100) is at least `threshold` (default 85). `normalize_case`/`normalize_spacing` fold case and whitespace before comparing. Each cluster's canonical suggestion is its most frequent original value. `output` is markdown (clusters + mapping table), csv (a flat original→canonical mapping), or json. Runs locally.",
        parameters = schema_json()
    )
)]
impl ClusterSimilarValues {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cluster-similar-values", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            cluster(
                &a.data,
                &a.column,
                &delim,
                a.header,
                a.threshold,
                a.normalize_case,
                a.normalize_spacing,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":              { "type": "string", "description": "The values to cluster: a CSV table, or a plain list with one value per line." },
                    "column":            { "type": "string", "default": "", "description": "Which column to cluster: a header name (needs header=true) or a 1-based index. Blank uses the only column (e.g. a newline list)." },
                    "delimiter":         { "type": "string", "default": ",", "description": "Field separator for CSV: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','; harmless for a newline list." },
                    "header":            { "type": "boolean", "default": false, "description": "Treat the first row as a header (skipped from clustering, and usable to name the column). Default false — leave off for a plain list of values." },
                    "threshold":         { "type": "integer", "minimum": 0, "maximum": 100, "default": 85, "description": "Similarity cutoff 0–100 (edit-distance ratio). Values at or above it join the same cluster. Higher = stricter/tighter clusters. Default 85." },
                    "normalize_case":    { "type": "boolean", "default": true, "description": "Ignore letter case when comparing (so 'USA' and 'usa' match). Default true." },
                    "normalize_spacing": { "type": "boolean", "default": true, "description": "Collapse and trim whitespace when comparing (so 'New  York' and 'New York' match). Default true." },
                    "output":            { "type": "string", "enum": ["markdown", "csv", "json"], "default": "markdown", "description": "Result format: markdown (clusters + a mapping table), csv (a flat original→canonical mapping), or json (structured clusters)." }
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
