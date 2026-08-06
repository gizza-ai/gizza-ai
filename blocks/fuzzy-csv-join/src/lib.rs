//! gizza-ai/fuzzy-csv-join — approximate-key joins for two CSV tables.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fuzzy_csv_join_core::fuzzy_join;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    left_key: String,
    #[serde(default)]
    right_key: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_join_type")]
    join_type: String,
    #[serde(default = "default_max_matches")]
    max_matches: usize,
    #[serde(default = "default_true")]
    show_score: bool,
    #[serde(default = "default_true")]
    normalize_case: bool,
    #[serde(default)]
    ignore_punctuation: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_algorithm() -> String {
    "jaro_winkler".into()
}
fn default_threshold() -> f64 {
    85.0
}
fn default_join_type() -> String {
    "inner".into()
}
fn default_max_matches() -> usize {
    1
}
fn default_true() -> bool {
    true
}
fn default_delimiter() -> String {
    ",".into()
}
fn default_output() -> String {
    "csv".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("left").required().describe("Left CSV text. The first row is the header and data rows are matched against the right CSV."))
        .param(Param::string("right").required().describe("Right CSV text. The first row is the header; matched rows are appended after the left columns."))
        .param(Param::string("left_key").required().describe("Key column in the left CSV, as a header name or 1-based column index."))
        .param(Param::string("right_key").default("").describe("Key column in the right CSV, as a header name or 1-based index. Blank reuses left_key's reference."))
        .param(Param::enumv("algorithm", ["jaro_winkler", "levenshtein", "token_sort", "soundex"]).default("jaro_winkler").describe("Similarity algorithm. jaro_winkler (default) handles abbreviations well; levenshtein is edit-distance ratio; token_sort ignores word order; soundex is phonetic."))
        .param(Param::number("threshold").default(85.0).min(0.0).max(100.0).describe("Minimum similarity score from 0 to 100, inclusive. Default 85."))
        .param(Param::enumv("join_type", ["inner", "left", "right", "outer"]).default("inner").describe("Rows to keep in the joined CSV: inner matches only; left keeps all left rows; right keeps all right rows; outer keeps both sides."))
        .param(Param::integer("max_matches").default(1).min(1.0).max(100.0).describe("Maximum right-side candidates emitted for each left row, best scores first. Default 1."))
        .param(Param::boolean("show_score").default(true).describe("Append a match_score column to joined CSV output. Default true."))
        .param(Param::boolean("normalize_case").default(true).describe("Compare keys case-insensitively by lowercasing before scoring. Default true."))
        .param(Param::boolean("ignore_punctuation").default(false).describe("Remove punctuation and symbols before scoring, so 'ACME, Inc.' and 'Acme Inc' compare closer. Default false."))
        .param(Param::string("delimiter").default(",").describe("CSV delimiter: a single character or comma/tab/semicolon/pipe. Default comma."))
        .param(Param::enumv("output", ["csv", "unmatched_left", "unmatched_right", "json"]).default("csv").describe("Return the joined CSV, only unmatched left rows, only unmatched right rows, or a JSON report with matches and coverage stats."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-csv-join",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Join two CSVs with fuzzy key matching",
    skill(
        description = "Join two CSV tables on approximately matching key values. Choose a similarity algorithm and threshold, emit SQL-style inner/left/right/outer joins, cap multiple candidate matches per left row, include match scores, and inspect unmatched rows or a JSON coverage report. Inputs are CSV text with header rows and all processing runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fuzzy-csv-join", |a: Args| {
            fuzzy_join(
                &a.left,
                &a.right,
                &a.left_key,
                &a.right_key,
                &a.algorithm,
                a.threshold,
                &a.join_type,
                a.max_matches,
                a.show_score,
                a.normalize_case,
                a.ignore_punctuation,
                &a.delimiter,
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
    fn schema_json_matches_descriptor_contract() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .unwrap();
        assert_eq!(
            schema["required"],
            serde_json::json!(["left", "right", "left_key"])
        );
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            props["algorithm"]["enum"],
            serde_json::json!(["jaro_winkler", "levenshtein", "token_sort", "soundex"])
        );
        assert_eq!(
            props["join_type"]["enum"],
            serde_json::json!(["inner", "left", "right", "outer"])
        );
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["csv", "unmatched_left", "unmatched_right", "json"])
        );
        assert_eq!(props["threshold"]["minimum"].as_f64(), Some(0.0));
        assert_eq!(props["threshold"]["maximum"].as_f64(), Some(100.0));
        for name in [
            "left",
            "right",
            "left_key",
            "right_key",
            "threshold",
            "show_score",
        ] {
            assert!(
                props[name].get("description").is_some(),
                "{name} needs a description"
            );
        }
    }
}
