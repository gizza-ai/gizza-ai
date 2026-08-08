//! gizza-ai/cumulative-percent-builder — Pareto cumulative percentage table.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default)]
    top_n: f64,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_delimiter() -> String {
    "auto".into()
}
fn default_header() -> String {
    "auto".into()
}
fn default_sort() -> String {
    "desc".into()
}
fn default_threshold() -> f64 {
    80.0
}
fn default_decimals() -> f64 {
    1.0
}
fn default_output() -> String {
    "table".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Rows to rank for Pareto/cumulative-percent analysis. Paste one label and one non-negative value per row, such as category,count. Delimiters can be comma, tab, semicolon, pipe, or whitespace. Up to 10000 rows."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe("How to split each row. Auto chooses the most common delimiter per row and falls back to whitespace. Default auto."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first non-empty row is a header. Auto skips it when the value column is not numeric. Default auto."))
        .param(Param::enumv("sort", ["desc", "input"]).default("desc").describe("Sort descending by value before building cumulative percentages, or keep input order. Pareto analysis normally uses desc. Default desc."))
        .param(Param::number("threshold").default(80.0).min(0.0).max(100.0).describe("Cumulative percentage cutoff for the vital-few zone. Rows up to and including the first row that crosses this threshold are marked vital. Default 80."))
        .param(Param::integer("top_n").default(0).min(0.0).max(200.0).describe("Optional tail bucketing: keep the top N rows and combine the rest into an Other row. 0 disables bucketing. Default 0."))
        .param(Param::integer("decimals").default(1).min(0.0).max(6.0).describe("Decimal places for values and percentages, from 0 to 6. Default 1."))
        .param(Param::enumv("output", ["table", "csv", "markdown"]).default("table").describe("Output format: aligned text table with a text Pareto chart, CSV, or Markdown table. Default table."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn whole(name: &str, v: f64, min: usize, max: usize) -> Result<usize, SkillError> {
    if !v.is_finite() || v.fract() != 0.0 || v < min as f64 || v > max as f64 {
        return Err(SkillError::InvalidArgs(format!(
            "{name} must be a whole number between {min} and {max}"
        )));
    }
    Ok(v as usize)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cumulative-percent-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort values and add running totals, cumulative percentages and Pareto vital-few labels",
    skill(
        description = "Build a Pareto-ready cumulative percentage table from pasted label,value rows. The tool can auto-detect delimiters, skip a header row, sort descending, compute percent of total, cumulative count, cumulative sum, cumulative percent, and mark the vital-few rows that reach a configurable threshold (80% by default). It can optionally bucket the tail into an Other row and emit aligned text, CSV, or Markdown. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cumulative-percent-builder", |a: Args| {
            let top_n = whole("top_n", a.top_n, 0, 200)?;
            let decimals = whole("decimals", a.decimals, 0, 6)?;
            gizza_ai_cumulative_percent_builder_core::run(
                &a.data,
                &a.delimiter,
                &a.header,
                &a.sort,
                a.threshold,
                top_n,
                decimals,
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
    fn every_param_is_documented() {
        for p in descriptor().params {
            assert!(
                !p.description.is_empty(),
                "param {} needs a describe()",
                p.name
            );
        }
    }
    #[test]
    fn schema_includes_required_and_enums() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["data"]));
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["table", "csv", "markdown"])
        );
        assert_eq!(
            v["properties"]["threshold"]["minimum"],
            serde_json::json!(0)
        );
    }
}
