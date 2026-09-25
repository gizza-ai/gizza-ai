//! gizza-ai/vector-similarity — compare a query vector to labelled candidates.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    vectors: String,
    #[serde(default = "default_metric")]
    metric: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
    #[serde(default)]
    normalize: bool,
    #[serde(default)]
    hamming_tolerance: f64,
    #[serde(default = "default_decimals")]
    decimals: usize,
    #[serde(default = "default_true")]
    show_all_metrics: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_metric() -> String { "cosine".to_string() }
fn default_top_k() -> usize { 5 }
fn default_decimals() -> usize { 6 }
fn default_true() -> bool { true }
fn default_output() -> String { "table".to_string() }

const METRICS: [&str; 7] = ["cosine", "cosine_distance", "dot", "euclidean", "manhattan", "chebyshev", "hamming"];
const OUTPUTS: [&str; 3] = ["table", "json", "csv"];

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("query").required().describe("Query vector to compare against, as numbers separated by commas, spaces or JSON-array punctuation. Example: 3, 2, 1."))
        .param(Param::string("vectors").required().describe("Candidate vectors, one per line. Prefix a label with 'label: values'; otherwise labels v1, v2, ... are assigned. Every vector must have the same number of dimensions as query."))
        .param(Param::enumv("metric", METRICS).default("cosine").describe("Ranking metric: cosine similarity, cosine_distance, dot product, Euclidean, Manhattan, Chebyshev or Hamming distance. Similarity metrics rank higher first; distance metrics rank lower first."))
        .param(Param::integer("top_k").default(5).min(1.0).max(2000.0).describe("Maximum number of nearest neighbours to return, from 1 to 2000. Values above the candidate count return every candidate."))
        .param(Param::boolean("normalize").default(false).describe("L2-normalize the query and each candidate before scoring. Useful when vector direction matters more than magnitude; zero vectors then fail clearly."))
        .param(Param::number("hamming_tolerance").default(0.0).min(0.0).describe("For Hamming distance, coordinates whose absolute difference is at most this tolerance count as equal. Default 0 for exact numeric equality."))
        .param(Param::integer("decimals").default(6).min(0.0).max(12.0).describe("Decimal places for floating-point scores in table, CSV and JSON output, from 0 to 12. Hamming counts remain integers."))
        .param(Param::boolean("show_all_metrics").default(true).describe("Include cosine, dot, Euclidean, Manhattan, Chebyshev and Hamming columns beside the ranking metric. Turn off for a compact result."))
        .param(Param::enumv("output", OUTPUTS).default("table").describe("Output format: aligned text table (default), JSON or CSV."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vector-similarity",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rank nearest vectors by cosine, dot product and distance metrics.",
    skill(
        description = "Compare a query vector with a list of candidate vectors and return the nearest neighbours. Accepts comma, whitespace or JSON-array-like numeric vectors; candidates may be labelled with `label: values`. metric chooses cosine, cosine_distance, dot, euclidean, manhattan, chebyshev or hamming. top_k limits returned neighbours, normalize optionally scales vectors to unit length, hamming_tolerance treats close numeric coordinates as equal, decimals controls score precision, show_all_metrics includes the companion metric columns, and output selects table, JSON or CSV. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vector-similarity", |a: Args| {
            gizza_ai_vector_similarity_core::run(
                &a.query,
                &a.vectors,
                &a.metric,
                a.top_k,
                a.normalize,
                a.hamming_tolerance,
                a.decimals,
                a.show_all_metrics,
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

    #[test]
    fn schema_has_expected_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in ["query", "vectors", "metric", "top_k", "normalize", "hamming_tolerance", "decimals", "show_all_metrics", "output"] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(props[key]["description"].as_str().unwrap_or_default().len() > 20);
        }
        assert_eq!(schema["required"], serde_json::json!(["query", "vectors"]));
        assert_eq!(props["metric"]["enum"], serde_json::json!(METRICS));
        assert_eq!(props["metric"]["default"], "cosine");
        assert_eq!(props["top_k"]["default"], 5);
        assert_eq!(props["normalize"]["default"], false);
        assert_eq!(props["show_all_metrics"]["default"], true);
        assert_eq!(props["output"]["enum"], serde_json::json!(OUTPUTS));
    }

    #[test]
    fn manifest_tool_parameters_match_the_descriptor() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(manifest["tool"]["parameters"], live);
    }
}
