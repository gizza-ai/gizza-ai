//! gizza-ai/data-drift-summary — per-column drift report between two tabular datasets.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    reference: String,
    current: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_bins")]
    bins: i64,
    #[serde(default = "default_max_categories")]
    max_categories: i64,
    #[serde(default = "default_drift_share")]
    drift_share: f64,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_delimiter() -> String {
    "comma".to_string()
}
fn default_true() -> bool {
    true
}
fn default_method() -> String {
    "psi".to_string()
}
fn default_threshold() -> f64 {
    0.2
}
fn default_bins() -> i64 {
    10
}
fn default_max_categories() -> i64 {
    20
}
fn default_drift_share() -> f64 {
    0.5
}
fn default_sort() -> String {
    "drift".to_string()
}
fn default_format() -> String {
    "table".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("reference").required().describe("The baseline (older) dataset as delimited text — for example the training set or last month's export. Example: id,amount,country\\n1,10,US"))
        .param(Param::string("current").required().describe("The new dataset to check for drift, with the same columns as the reference. Example: id,amount,country\\n1,12,DE"))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter used by both datasets. Default comma."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header and match columns by name, so reordered columns still line up. Turn off to match by position (col1, col2, …)."))
        .param(Param::string("columns").default("").describe("Comma-separated allow-list of columns to report on, e.g. amount,country. Leave empty to report on every column present in both datasets."))
        .param(Param::enumv("method", ["psi", "jsd"]).default("psi").describe("Drift metric: psi = Population Stability Index (unbounded, 0.1 moderate / 0.2 significant), jsd = Jensen-Shannon distance (0 to 1, 0.1 is a common cutoff). Default psi."))
        .param(Param::number("threshold").default(0.2).min(0.0).describe("A column counts as drifted when its score reaches this value. Default 0.2 (the usual PSI significant-shift line); use 0.1 for jsd or for a stricter PSI check."))
        .param(Param::integer("bins").default(10).min(2.0).max(50.0).describe("Number of equal-population bins used to compare numeric columns, derived from the reference values. Default 10. More bins are more sensitive to small shifts."))
        .param(Param::integer("max_categories").default(20).min(2.0).max(1000.0).describe("A numeric column with more than this many distinct reference values is binned as a distribution; anything at or below it is compared as categories. Default 20."))
        .param(Param::number("drift_share").default(0.5).min(0.0).max(1.0).describe("Share of compared columns that must drift before the whole dataset is flagged. Default 0.5, i.e. half the columns."))
        .param(Param::boolean("ignore_case").default(false).describe("Trim whitespace and fold case before comparing category values, so 'New York' and 'new york ' count as the same value instead of a new category."))
        .param(Param::enumv("sort", ["drift", "name", "order"]).default("drift").describe("Column order in the report: drift (highest score first, the default), name (alphabetical), or order (as they appear in the reference)."))
        .param(Param::enumv("format", ["table", "markdown", "json", "csv"]).default("table").describe("Output: table (readable report), markdown (report table you can paste into a PR), json (structured report), or csv (one row per column)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-drift-summary",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two tabular datasets and report per-column type, null-rate, cardinality, range and category drift",
    skill(
        description = "Compare a reference (baseline) dataset against a current one, column by column, and report what changed: the inferred type on each side and whether it flipped, the null/missing rate and its delta, the distinct-value count, the numeric range and mean, the category values that are new in the current data and the ones that went missing, and a distribution drift score per column (Population Stability Index or Jensen-Shannon distance). Columns present on only one side are reported as schema drift, and a dataset-level verdict fires once a configurable share of columns has drifted. Accepts CSV/TSV text with comma, tab, semicolon or pipe delimiters, with or without a header row; outputs a readable table, a markdown report, structured JSON, or one CSV row per column.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-drift-summary", |a: Args| {
            gizza_ai_data_drift_summary_core::run(
                &a.reference,
                &a.current,
                &a.delimiter,
                a.header,
                &a.columns,
                &a.method,
                a.threshold,
                a.bins.clamp(0, 10_000) as usize,
                a.max_categories.clamp(0, 100_000) as usize,
                a.drift_share,
                a.ignore_case,
                &a.sort,
                &a.format,
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
                "reference": { "type": "string", "description": "The baseline (older) dataset as delimited text — for example the training set or last month's export. Example: id,amount,country\\n1,10,US" },
                "current": { "type": "string", "description": "The new dataset to check for drift, with the same columns as the reference. Example: id,amount,country\\n1,12,DE" },
                "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter used by both datasets. Default comma." },
                "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header and match columns by name, so reordered columns still line up. Turn off to match by position (col1, col2, …)." },
                "columns": { "type": "string", "default": "", "description": "Comma-separated allow-list of columns to report on, e.g. amount,country. Leave empty to report on every column present in both datasets." },
                "method": { "type": "string", "enum": ["psi", "jsd"], "default": "psi", "description": "Drift metric: psi = Population Stability Index (unbounded, 0.1 moderate / 0.2 significant), jsd = Jensen-Shannon distance (0 to 1, 0.1 is a common cutoff). Default psi." },
                "threshold": { "type": "number", "default": 0.2, "minimum": 0, "description": "A column counts as drifted when its score reaches this value. Default 0.2 (the usual PSI significant-shift line); use 0.1 for jsd or for a stricter PSI check." },
                "bins": { "type": "integer", "default": 10, "minimum": 2, "maximum": 50, "description": "Number of equal-population bins used to compare numeric columns, derived from the reference values. Default 10. More bins are more sensitive to small shifts." },
                "max_categories": { "type": "integer", "default": 20, "minimum": 2, "maximum": 1000, "description": "A numeric column with more than this many distinct reference values is binned as a distribution; anything at or below it is compared as categories. Default 20." },
                "drift_share": { "type": "number", "default": 0.5, "minimum": 0, "maximum": 1, "description": "Share of compared columns that must drift before the whole dataset is flagged. Default 0.5, i.e. half the columns." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Trim whitespace and fold case before comparing category values, so 'New York' and 'new york ' count as the same value instead of a new category." },
                "sort": { "type": "string", "enum": ["drift", "name", "order"], "default": "drift", "description": "Column order in the report: drift (highest score first, the default), name (alphabetical), or order (as they appear in the reference)." },
                "format": { "type": "string", "enum": ["table", "markdown", "json", "csv"], "default": "table", "description": "Output: table (readable report), markdown (report table you can paste into a PR), json (structured report), or csv (one row per column)." }
            },
            "required": ["reference", "current"],
            "additionalProperties": false
        }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
