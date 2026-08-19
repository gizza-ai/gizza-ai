//! gizza-ai/train-test-split — split a CSV into train / test (and an optional
//! validation) set, with stratification, grouped splits and a reproducible seed.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_train_test_split_core::split;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_test_size")]
    test_size: f64,
    #[serde(default)]
    validation_size: f64,
    #[serde(default)]
    stratify_column: String,
    #[serde(default)]
    stratify_bins: u64,
    #[serde(default)]
    group_column: String,
    #[serde(default = "default_true")]
    shuffle: bool,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delim")]
    delimiter: String,
    #[serde(default = "default_output")]
    output: String,
}
fn default_test_size() -> f64 { 0.2 }
fn default_seed() -> u64 { 42 }
fn default_true() -> bool { true }
fn default_delim() -> String { "comma".into() }
fn default_output() -> String { "sections".into() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to split into train and test sets."))
        .param(Param::number("test_size").default(0.2).min(0.0).describe("Size of the test set: below 1 it is a proportion of the data rows (0.2 = 20%), 1 or more is an absolute row count (50 = 50 rows). Default 0.2."))
        .param(Param::number("validation_size").default(0.0).min(0.0).describe("Size of an optional third validation set, same convention as test_size (below 1 = proportion, 1 or more = row count). 0 means no validation set. Default 0."))
        .param(Param::string("stratify_column").default("").describe("Keep this column's class balance identical in every split — a header name (header=true) or a 1-based index, e.g. 'label'. Requires shuffle=true and cannot be combined with group_column. Empty means no stratification."))
        .param(Param::integer("stratify_bins").default(0).min(0.0).max(100.0).describe("For a NUMERIC stratify_column: bin the values into this many equal-count (quantile) strata before splitting, e.g. 4 for quartiles. 0 uses the raw values as classes. Default 0."))
        .param(Param::string("group_column").default("").describe("Keep every row sharing a value in this column in the SAME split, so related rows (a patient, a user, a document) can't leak across sets — a header name or a 1-based index. Cannot be combined with stratify_column. Empty means no grouping."))
        .param(Param::boolean("shuffle").default(true).describe("Shuffle rows before splitting. Turn OFF for a sequential/time-series split: train takes the first rows, then validation, then test takes the last rows. Default true."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the reproducible PRNG — the same seed always produces the same split. Change it for a different draw. Default 42."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (repeated at the top of every split, and used to resolve column names). Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter of the input, also used for the output. Default comma."))
        .param(Param::enumv("output", ["sections", "train", "test", "validation", "summary", "json"]).default("sections").describe("What to return: sections (every split, each under a '# train (N rows)' label), train/test/validation (that one split's CSV alone), summary (row counts, percentages and the per-class balance), or json (all splits plus counts as a JSON object). Default sections."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct TrainTestSplit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/train-test-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a CSV into train/test (and validation) sets with stratification and a reproducible seed",
    skill(
        description = "Split a CSV's data rows into a train and a test set, plus an optional validation set. `test_size`/`validation_size` are a proportion when below 1 (0.2 = 20%) or an absolute row count when 1 or more. `stratify_column` keeps a label column's class balance identical across the splits (use `stratify_bins` to quantile-bin a numeric column first); `group_column` instead keeps all rows sharing a value together so related rows can't leak across splits. Turn `shuffle` off for a sequential time-series split (train first, test last). Every draw is reproducible via `seed`. `output` picks sections (all splits labelled), one split alone, a summary of counts and class balance, or json.",
        parameters = schema_json()
    ),
)]
impl TrainTestSplit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "train-test-split", |a: Args| {
            split(
                &a.data,
                a.test_size,
                a.validation_size,
                &a.stratify_column,
                a.stratify_bins as usize,
                &a.group_column,
                a.shuffle,
                a.seed,
                a.header,
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":            { "type": "string", "description": "The CSV text to split into train and test sets." },
                    "test_size":       { "type": "number", "default": 0.2, "minimum": 0, "description": "Size of the test set: below 1 it is a proportion of the data rows (0.2 = 20%), 1 or more is an absolute row count (50 = 50 rows). Default 0.2." },
                    "validation_size": { "type": "number", "default": 0.0, "minimum": 0, "description": "Size of an optional third validation set, same convention as test_size (below 1 = proportion, 1 or more = row count). 0 means no validation set. Default 0." },
                    "stratify_column": { "type": "string", "default": "", "description": "Keep this column's class balance identical in every split — a header name (header=true) or a 1-based index, e.g. 'label'. Requires shuffle=true and cannot be combined with group_column. Empty means no stratification." },
                    "stratify_bins":   { "type": "integer", "default": 0, "minimum": 0, "maximum": 100, "description": "For a NUMERIC stratify_column: bin the values into this many equal-count (quantile) strata before splitting, e.g. 4 for quartiles. 0 uses the raw values as classes. Default 0." },
                    "group_column":    { "type": "string", "default": "", "description": "Keep every row sharing a value in this column in the SAME split, so related rows (a patient, a user, a document) can't leak across sets — a header name or a 1-based index. Cannot be combined with stratify_column. Empty means no grouping." },
                    "shuffle":         { "type": "boolean", "default": true, "description": "Shuffle rows before splitting. Turn OFF for a sequential/time-series split: train takes the first rows, then validation, then test takes the last rows. Default true." },
                    "seed":            { "type": "integer", "default": 42, "minimum": 0, "description": "Seed for the reproducible PRNG — the same seed always produces the same split. Change it for a different draw. Default 42." },
                    "header":          { "type": "boolean", "default": true, "description": "Treat the first row as a header (repeated at the top of every split, and used to resolve column names). Default true." },
                    "delimiter":       { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter of the input, also used for the output. Default comma." },
                    "output":          { "type": "string", "enum": ["sections", "train", "test", "validation", "summary", "json"], "default": "sections", "description": "What to return: sections (every split, each under a '# train (N rows)' label), train/test/validation (that one split's CSV alone), summary (row counts, percentages and the per-class balance), or json (all splits plus counts as a JSON object). Default sections." }
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
