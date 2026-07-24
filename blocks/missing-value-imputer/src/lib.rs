//! gizza-ai/missing-value-imputer — fill missing cells in a CSV with mean,
//! median, most-frequent, constant, or KNN (nan-euclidean) imputation. Thin
//! wrapper around the core; the chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_missing_value_imputer_core::impute;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    na_tokens: String,
    #[serde(default)]
    fill_value: String,
    #[serde(default = "default_neighbors")]
    n_neighbors: u32,
    #[serde(default = "default_weights")]
    weights: String,
}
fn default_true() -> bool { true }
fn default_strategy() -> String { "mean".to_string() }
fn default_delimiter() -> String { "comma".to_string() }
fn default_neighbors() -> u32 { 5 }
fn default_weights() -> String { "uniform".to_string() }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The CSV text with missing cells to fill."))
        .param(Param::enumv("strategy", ["mean", "median", "most_frequent", "constant", "knn"]).default("mean").describe("How to fill each missing cell: 'mean'/'median' (numeric columns only), 'most_frequent' (any column, ties → first seen), 'constant' (write fill_value), or 'knn' (nan-euclidean nearest-neighbour average of numeric columns). Default 'mean'."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it verbatim and use its names for the 'columns' selector. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'."))
        .param(Param::string("columns").describe("Comma-separated columns to impute — header names (needs a header) or 1-based indexes (e.g. 'age,income' or '2,3'). Blank imputes every applicable column."))
        .param(Param::string("na_tokens").describe("Comma-separated extra strings that count as missing besides blank cells (e.g. 'NA,null,?'). Case-sensitive, matched after trimming."))
        .param(Param::string("fill_value").describe("The value written into missing cells when strategy='constant' (e.g. '0' or 'Unknown'). Ignored by other strategies."))
        .param(Param::integer("n_neighbors").default(5).min(1.0).max(100.0).describe("Neighbours averaged by strategy='knn'. Default 5."))
        .param(Param::enumv("weights", ["uniform", "distance"]).default("uniform").describe("KNN neighbour weighting: 'uniform' (plain average) or 'distance' (inverse-distance weighted). Default 'uniform'."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct MissingValueImputer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/missing-value-imputer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fill missing CSV cells by mean, median, most-frequent, constant, or KNN",
    skill(
        description = "Fill missing cells in a CSV. A cell is missing when it is blank or equals one of the caller's na_tokens (e.g. NA, null, ?). Strategies: 'mean'/'median' impute numeric columns only; 'most_frequent' works on any column; 'constant' writes fill_value; 'knn' averages the nan-euclidean nearest neighbours over the numeric columns. Restrict to specific columns by header name or 1-based index, or leave blank to impute every applicable column. Delimiters accept comma/tab/semicolon/pipe.",
        parameters = schema_json()
    ),
)]
impl MissingValueImputer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "missing-value-imputer", |a: Args| {
            let strategy = if a.strategy.is_empty() { "mean".to_string() } else { a.strategy };
            let delimiter = if a.delimiter.is_empty() { "comma".to_string() } else { a.delimiter };
            let weights = if a.weights.is_empty() { "uniform".to_string() } else { a.weights };
            impute(
                &a.input,
                a.header,
                &delimiter,
                &strategy,
                &a.columns,
                &a.na_tokens,
                &a.fill_value,
                a.n_neighbors.max(1) as usize,
                &weights,
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
                    "input":       { "type": "string", "description": "The CSV text with missing cells to fill." },
                    "strategy":    { "type": "string", "enum": ["mean", "median", "most_frequent", "constant", "knn"], "default": "mean", "description": "How to fill each missing cell: 'mean'/'median' (numeric columns only), 'most_frequent' (any column, ties → first seen), 'constant' (write fill_value), or 'knn' (nan-euclidean nearest-neighbour average of numeric columns). Default 'mean'." },
                    "header":      { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it verbatim and use its names for the 'columns' selector. Default true." },
                    "delimiter":   { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'." },
                    "columns":     { "type": "string", "description": "Comma-separated columns to impute — header names (needs a header) or 1-based indexes (e.g. 'age,income' or '2,3'). Blank imputes every applicable column." },
                    "na_tokens":   { "type": "string", "description": "Comma-separated extra strings that count as missing besides blank cells (e.g. 'NA,null,?'). Case-sensitive, matched after trimming." },
                    "fill_value":  { "type": "string", "description": "The value written into missing cells when strategy='constant' (e.g. '0' or 'Unknown'). Ignored by other strategies." },
                    "n_neighbors": { "type": "integer", "minimum": 1, "maximum": 100, "default": 5, "description": "Neighbours averaged by strategy='knn'. Default 5." },
                    "weights":     { "type": "string", "enum": ["uniform", "distance"], "default": "uniform", "description": "KNN neighbour weighting: 'uniform' (plain average) or 'distance' (inverse-distance weighted). Default 'uniform'." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
