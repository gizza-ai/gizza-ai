//! gizza-ai/class-rebalancer — balance an imbalanced CSV label column by random
//! over-sampling the minority class(es) and/or under-sampling the majority
//! class(es) toward a target ratio, with a fixed seed. Chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_class_rebalancer_core::rebalance;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    label_column: String,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "default_ratio")]
    target_ratio: f64,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    shuffle: bool,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_output")]
    output: String,
}
fn default_strategy() -> String {
    "auto".into()
}
fn default_ratio() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_seed() -> u64 {
    42
}
fn default_output() -> String {
    "csv".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to rebalance. One column holds the class label; the others are kept verbatim on every row that is duplicated or dropped."))
        .param(Param::string("label_column").default("").describe("Which column holds the class label: a header name (when header=true) or a 1-based column number. Blank = the last column. Default blank (last column)."))
        .param(Param::enumv("strategy", ["auto", "oversample", "undersample", "combine"]).default("auto").describe("How to balance: oversample (randomly duplicate minority-class rows up), undersample (randomly drop majority-class rows down), combine (do both, moving every class to a common size), or auto (same as oversample). Default auto."))
        .param(Param::number("target_ratio").default(1.0).min(0.01).max(1.0).describe("Desired minority-to-majority class ratio after resampling, from just above 0 to 1.0. 1.0 = fully balanced (every class equal); 0.5 = the smaller class ends at half the larger. Default 1.0."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (kept in the output and used to resolve label_column names). Default true."))
        .param(Param::boolean("shuffle").default(false).describe("Shuffle the output rows with the seeded PRNG. When false, original rows keep their file order and duplicated rows are appended at the end. Default false."))
        .param(Param::integer("seed").default(42).min(0.0).describe("Seed for the reproducible PRNG used to pick which rows to duplicate/drop and to shuffle. Same seed → same result; change it for a different draw. Default 42."))
        .param(Param::enumv("output", ["csv", "summary"]).default("csv").describe("What to return: csv (the rebalanced CSV) or summary (a JSON report of each class's before/after count and the totals). Default csv."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/class-rebalancer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rebalance an imbalanced CSV label column by seeded over/under-sampling",
    skill(
        description = "Balance an imbalanced label column of a CSV by random resampling of whole rows, with a fixed seed for reproducibility. `strategy` is oversample (duplicate minority-class rows up), undersample (drop majority-class rows down), combine (both, to a common size), or auto (= oversample). `target_ratio` (0–1] is the desired minority:majority ratio after resampling (1.0 = fully balanced). `label_column` is a header name or 1-based index (blank = last column). Returns the rebalanced CSV, or a JSON before/after count report when `output=summary`. Runs entirely locally; no synthetic rows are invented (this is random resampling, not SMOTE).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "class-rebalancer", |a: Args| {
            rebalance(
                &a.data,
                &a.label_column,
                &a.strategy,
                a.target_ratio,
                a.header,
                a.shuffle,
                a.seed,
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
                    "data":         { "type": "string", "description": "The CSV text to rebalance. One column holds the class label; the others are kept verbatim on every row that is duplicated or dropped." },
                    "label_column": { "type": "string", "default": "", "description": "Which column holds the class label: a header name (when header=true) or a 1-based column number. Blank = the last column. Default blank (last column)." },
                    "strategy":     { "type": "string", "enum": ["auto", "oversample", "undersample", "combine"], "default": "auto", "description": "How to balance: oversample (randomly duplicate minority-class rows up), undersample (randomly drop majority-class rows down), combine (do both, moving every class to a common size), or auto (same as oversample). Default auto." },
                    "target_ratio": { "type": "number", "default": 1.0, "minimum": 0.01, "maximum": 1, "description": "Desired minority-to-majority class ratio after resampling, from just above 0 to 1.0. 1.0 = fully balanced (every class equal); 0.5 = the smaller class ends at half the larger. Default 1.0." },
                    "header":       { "type": "boolean", "default": true, "description": "Treat the first row as a header (kept in the output and used to resolve label_column names). Default true." },
                    "shuffle":      { "type": "boolean", "default": false, "description": "Shuffle the output rows with the seeded PRNG. When false, original rows keep their file order and duplicated rows are appended at the end. Default false." },
                    "seed":         { "type": "integer", "default": 42, "minimum": 0, "description": "Seed for the reproducible PRNG used to pick which rows to duplicate/drop and to shuffle. Same seed → same result; change it for a different draw. Default 42." },
                    "output":       { "type": "string", "enum": ["csv", "summary"], "default": "csv", "description": "What to return: csv (the rebalanced CSV) or summary (a JSON report of each class's before/after count and the totals). Default csv." }
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
