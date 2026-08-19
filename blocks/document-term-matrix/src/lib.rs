//! gizza-ai/document-term-matrix — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_count")]
    weighting: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_one")]
    ngram_min: u32,
    #[serde(default = "default_one")]
    ngram_max: u32,
    #[serde(default = "default_one")]
    min_df: u32,
    #[serde(default)]
    max_features: u32,
    #[serde(default = "default_csv")]
    output: String,
    #[serde(default = "default_true")]
    include_totals: bool,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_count() -> String {
    "count".to_string()
}
fn default_csv() -> String {
    "csv".to_string()
}
fn default_one() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("documents").required().describe("Documents to vectorize. Use a JSON array of strings (for exact documents with embedded newlines) or one document per line. Blank lines are ignored in lines mode. Limit: 10,000 documents."))
        .param(Param::enumv("input_format", ["auto", "json", "lines"]).default("auto").describe("How to read documents: auto treats input starting with '[' as a JSON array of strings and anything else as one document per line; json requires a JSON array; lines reads each nonblank line as one document."))
        .param(Param::enumv("weighting", ["count", "binary"]).default("count").describe("Cell weighting: count stores term occurrence counts per document; binary stores 1 when the term appears at least once and 0 otherwise."))
        .param(Param::boolean("case_sensitive").default(false).describe("Keep uppercase and lowercase terms separate. Default false folds tokens to lowercase before counting."))
        .param(Param::integer("ngram_min").default(1).min(1.0).max(3.0).describe("Smallest n-gram length to include. 1 means individual terms; 2 adds adjacent two-word phrases; 3 adds three-word phrases. Must be 1-3 and <= ngram_max."))
        .param(Param::integer("ngram_max").default(1).min(1.0).max(3.0).describe("Largest n-gram length to include. Must be 1-3 and >= ngram_min. Use 1 for ordinary bag-of-words."))
        .param(Param::integer("min_df").default(1).min(1.0).max(100000.0).describe("Minimum document frequency: keep only terms that appear in at least this many documents. Raise it to remove rare terms from a large corpus."))
        .param(Param::integer("max_features").default(0).min(0.0).max(5000.0).describe("Maximum vocabulary columns to keep after sorting by document frequency then term. Use 0 for no cap up to the hard 5,000-column limit."))
        .param(Param::enumv("output", ["csv", "json", "tsv"]).default("csv").describe("Output format: csv (default) for spreadsheets, tsv for tab-separated matrices, or json for structured terms/document_frequency/matrix arrays."))
        .param(Param::boolean("include_totals").default(true).describe("Add a __total_terms column to delimited output and a total_terms array to JSON. With binary weighting this is the number of distinct kept terms in each document."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-term-matrix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build document-term matrices from pasted documents",
    skill(
        description = "Build a document-term matrix from a collection of documents. Paste one document per line or a JSON array of strings; tokenize words locally; optionally case-fold; include unigrams, bigrams or trigrams; filter vocabulary by minimum document frequency; cap max features; export count or binary bag-of-words matrices as CSV, TSV or JSON. Columns are ordered by descending document frequency then term. Limits: 10,000 documents, 5,000 terms, n-grams 1-3.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "document-term-matrix", |a: Args| {
            gizza_ai_document_term_matrix_core::run(
                &a.documents,
                &a.input_format,
                &a.weighting,
                a.case_sensitive,
                a.ngram_min,
                a.ngram_max,
                a.min_df,
                a.max_features,
                &a.output,
                a.include_totals,
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
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["required"], serde_json::json!(["documents"]));
        assert_eq!(v["properties"]["documents"]["type"], "string");
        assert_eq!(
            v["properties"]["input_format"]["enum"],
            serde_json::json!(["auto", "json", "lines"])
        );
        assert_eq!(
            v["properties"]["weighting"]["enum"],
            serde_json::json!(["count", "binary"])
        );
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["csv", "json", "tsv"])
        );
        assert_eq!(v["properties"]["ngram_min"]["minimum"], 1.0);
        assert_eq!(v["properties"]["ngram_max"]["maximum"], 3.0);
        assert_eq!(v["properties"]["max_features"]["maximum"], 5000.0);
        assert_eq!(v["properties"]["include_totals"]["default"], true);
        let props = v["properties"].as_object().unwrap();
        let mut names: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "case_sensitive",
                "documents",
                "include_totals",
                "input_format",
                "max_features",
                "min_df",
                "ngram_max",
                "ngram_min",
                "output",
                "weighting"
            ]
        );
        for (name, p) in props {
            assert!(
                p["description"].as_str().unwrap_or("").len() > 20,
                "param {name} needs a real description"
            );
        }
    }

    #[test]
    fn defaults_deserialize_from_documents_only() {
        let a: Args = serde_json::from_str(r#"{"documents":"cat dog\ndog"}"#).unwrap();
        assert_eq!(a.input_format, "auto");
        assert_eq!(a.weighting, "count");
        assert!(!a.case_sensitive);
        assert_eq!(a.ngram_min, 1);
        assert_eq!(a.ngram_max, 1);
        assert_eq!(a.min_df, 1);
        assert_eq!(a.max_features, 0);
        assert_eq!(a.output, "csv");
        assert!(a.include_totals);
    }
}
