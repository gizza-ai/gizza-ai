//! gizza-ai/data-normalize — scale the numeric columns of a CSV with min-max,
//! z-score (standard), or robust (median/IQR) normalization. Thin wrapper around
//! the core; the chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → runs on
//! all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_normalize_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_range_min")]
    range_min: f64,
    #[serde(default = "default_range_max")]
    range_max: f64,
    #[serde(default)]
    ddof: u32,
    #[serde(default = "default_true")]
    with_centering: bool,
    #[serde(default = "default_true")]
    with_scaling: bool,
}
fn default_true() -> bool { true }
fn default_method() -> String { "min_max".to_string() }
fn default_delimiter() -> String { "comma".to_string() }
fn default_range_min() -> f64 { 0.0 }
fn default_range_max() -> f64 { 1.0 }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The CSV text whose numeric columns should be scaled."))
        .param(Param::enumv("method", ["min_max", "z_score", "robust"]).default("min_max").describe("Scaling method: 'min_max' rescales each column into [range_min, range_max]; 'z_score' standardizes to mean 0 and unit stddev; 'robust' centers on the median and scales by the interquartile range. Default 'min_max'."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it verbatim and use its names for the 'columns' selector. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'."))
        .param(Param::string("columns").describe("Comma-separated columns to scale — header names (needs a header) or 1-based indexes (e.g. 'age,income' or '2,3'). Blank scales every numeric column."))
        .param(Param::number("range_min").default(0.0).describe("Lower bound of the target range for method='min_max'. Must be less than range_max. Default 0."))
        .param(Param::number("range_max").default(1.0).describe("Upper bound of the target range for method='min_max'. Must be greater than range_min. Default 1."))
        .param(Param::integer("ddof").default(0).min(0.0).max(1.0).describe("Delta degrees of freedom for the method='z_score' standard deviation: 0 = population, 1 = sample (Bessel-corrected). Default 0."))
        .param(Param::boolean("with_centering").default(true).describe("For method='robust', subtract the column median before scaling. Default true."))
        .param(Param::boolean("with_scaling").default(true).describe("For method='robust', divide by the interquartile range (IQR); a zero IQR divides by 1. Default true."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct DataNormalize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-normalize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scale numeric CSV columns with min-max, z-score, or robust normalization",
    skill(
        description = "Normalize (feature-scale) the numeric columns of a CSV. Methods: 'min_max' rescales each column into [range_min, range_max]; 'z_score' standardizes to mean 0 / unit stddev (ddof selects population vs sample stddev, and a zero stddev yields 0); 'robust' centers on the median and scales by the interquartile range (with_centering / with_scaling toggle each, and a zero IQR divides by 1). Only numeric columns (every present value parses as a finite number) are scaled; text columns are copied and blank cells stay blank. Restrict to specific columns by header name or 1-based index, or leave blank to scale every numeric column. Delimiters accept comma/tab/semicolon/pipe.",
        parameters = schema_json()
    ),
)]
impl DataNormalize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-normalize", |a: Args| {
            let method = if a.method.is_empty() { "min_max".to_string() } else { a.method };
            let delimiter = if a.delimiter.is_empty() { "comma".to_string() } else { a.delimiter };
            normalize(
                &a.input,
                a.header,
                &delimiter,
                &method,
                &a.columns,
                a.range_min,
                a.range_max,
                a.ddof,
                a.with_centering,
                a.with_scaling,
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
                    "input":          { "type": "string", "description": "The CSV text whose numeric columns should be scaled." },
                    "method":         { "type": "string", "enum": ["min_max", "z_score", "robust"], "default": "min_max", "description": "Scaling method: 'min_max' rescales each column into [range_min, range_max]; 'z_score' standardizes to mean 0 and unit stddev; 'robust' centers on the median and scales by the interquartile range. Default 'min_max'." },
                    "header":         { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it verbatim and use its names for the 'columns' selector. Default true." },
                    "delimiter":      { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'." },
                    "columns":        { "type": "string", "description": "Comma-separated columns to scale — header names (needs a header) or 1-based indexes (e.g. 'age,income' or '2,3'). Blank scales every numeric column." },
                    "range_min":      { "type": "number", "default": 0.0, "description": "Lower bound of the target range for method='min_max'. Must be less than range_max. Default 0." },
                    "range_max":      { "type": "number", "default": 1.0, "description": "Upper bound of the target range for method='min_max'. Must be greater than range_min. Default 1." },
                    "ddof":           { "type": "integer", "minimum": 0, "maximum": 1, "default": 0, "description": "Delta degrees of freedom for the method='z_score' standard deviation: 0 = population, 1 = sample (Bessel-corrected). Default 0." },
                    "with_centering": { "type": "boolean", "default": true, "description": "For method='robust', subtract the column median before scaling. Default true." },
                    "with_scaling":   { "type": "boolean", "default": true, "description": "For method='robust', divide by the interquartile range (IQR); a zero IQR divides by 1. Default true." }
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
