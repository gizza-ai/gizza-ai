//! gizza-ai/per-capita-normalizer — turn raw counts into population-normalized rates.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI
//! and the page query-params); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default = "default_per")]
    per: String,
    #[serde(default)]
    custom_per: f64,
    #[serde(default = "default_population_unit")]
    population_unit: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_unstable_below")]
    unstable_below: f64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_per() -> String {
    "100000".into()
}
fn default_population_unit() -> String {
    "ones".into()
}
fn default_sort() -> String {
    "rate_desc".into()
}
fn default_output() -> String {
    "table".into()
}
fn default_decimals() -> f64 {
    2.0
}
fn default_unstable_below() -> f64 {
    20.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The rows to normalize: one 'label, count, population' row per line, such as 'Northbridge,120,400000'. The last two fields are read as count and population, so a label may itself contain the delimiter ('Springfield, IL,10,1000'). A two-field row is read as 'count, population' and labelled row 1, row 2, … Fields can be separated by comma, tab, semicolon, pipe or whitespace, and numbers may carry $/£/€, thousands separators or underscores. Up to 10000 rows.",
        ))
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe(
                    "How to split each row into fields. 'auto' (default) picks the most common separator in the row and falls back to whitespace; the others force comma, tab, semicolon or pipe.",
                ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"])
                .default("auto")
                .describe(
                    "Whether the first non-empty row is a header. 'auto' (default) skips it when its count or population field is not numeric; 'yes' always skips it; 'no' treats every row as data.",
                ),
        )
        .param(
            Param::enumv("per", ["1", "1000", "10000", "100000", "1000000", "custom"])
                .default("100000")
                .describe(
                    "Reporting base the rate is scaled to: '1' = per person (per capita), '1000' = per 1,000, '10000' = per 10,000, '100000' = per 100,000 (default, the public-health convention), '1000000' = per 1,000,000, or 'custom' to use the custom_per value.",
                ),
        )
        .param(
            Param::number("custom_per")
                .default(0.0)
                .min(0.0)
                .max(1_000_000_000_000.0)
                .describe(
                    "Custom reporting base, used only when per='custom' — for example 500 for 'per 500 residents'. Must be greater than 0 in that case. Ignored otherwise. Default 0.",
                ),
        )
        .param(
            Param::enumv("population_unit", ["ones", "thousands", "millions"])
                .default("ones")
                .describe(
                    "The unit the population column is expressed in. 'ones' (default) = actual people; 'thousands' multiplies it by 1,000 (World-Bank-style tables); 'millions' multiplies it by 1,000,000.",
                ),
        )
        .param(
            Param::integer("decimals")
                .default(2)
                .min(0.0)
                .max(6.0)
                .describe("Decimal places for the rate and the overall rate, from 0 to 6. Default 2."),
        )
        .param(
            Param::enumv("sort", ["rate_desc", "rate_asc", "input"])
                .default("rate_desc")
                .describe(
                    "Row order in the report: 'rate_desc' (default) highest rate first, 'rate_asc' lowest first, or 'input' to keep the pasted order. Ties are broken by label.",
                ),
        )
        .param(
            Param::integer("unstable_below")
                .default(20)
                .min(0.0)
                .max(1_000_000.0)
                .describe(
                    "Flag rows whose raw count is below this number as 'unstable', the convention that rates built on very few events are unreliable. Default 20; set 0 to flag nothing (every row then reads 'ok').",
                ),
        )
        .param(
            Param::enumv("output", ["table", "csv", "markdown", "json"])
                .default("table")
                .describe(
                    "Output format: 'table' (default) tab-separated columns plus a text rate chart, 'csv' a spreadsheet-ready file with a metric header block, 'markdown' a report table, or 'json' structured rows.",
                ),
        )
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
    name = "gizza-ai/per-capita-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn counts into per-capita, per-1,000 or per-100,000 rates using a population column",
    skill(
        description = "Normalize raw counts into population-adjusted rates so regions of different sizes can be compared. Paste 'label, count, population' rows and the tool divides each count by its population and scales it to a reporting base — per person, per 1,000, per 10,000, per 100,000 (default), per 1,000,000, or a custom base. It ranks rows by rate, adds an index against the overall rate (1.00 = the combined average), flags rows built on fewer than 20 events as statistically unstable, handles population columns written in thousands or millions, and emits a text table with a rate chart, CSV, Markdown, or JSON. Runs locally with no data upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "per-capita-normalizer", |a: Args| {
            let decimals = whole("decimals", a.decimals, 0, 6)?;
            let unstable_below = whole("unstable_below", a.unstable_below, 0, 1_000_000)?;
            gizza_ai_per_capita_normalizer_core::run(
                &a.data,
                &a.delimiter,
                &a.header,
                &a.per,
                a.custom_per,
                &a.population_unit,
                decimals,
                &a.sort,
                unstable_below as f64,
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
              "type": "object",
              "additionalProperties": false,
              "required": ["data"],
              "properties": {
                "data": {
                  "type": "string",
                  "description": "The rows to normalize: one 'label, count, population' row per line, such as 'Northbridge,120,400000'. The last two fields are read as count and population, so a label may itself contain the delimiter ('Springfield, IL,10,1000'). A two-field row is read as 'count, population' and labelled row 1, row 2, … Fields can be separated by comma, tab, semicolon, pipe or whitespace, and numbers may carry $/£/€, thousands separators or underscores. Up to 10000 rows."
                },
                "delimiter": {
                  "type": "string",
                  "enum": ["auto", "comma", "tab", "semicolon", "pipe"],
                  "default": "auto",
                  "description": "How to split each row into fields. 'auto' (default) picks the most common separator in the row and falls back to whitespace; the others force comma, tab, semicolon or pipe."
                },
                "header": {
                  "type": "string",
                  "enum": ["auto", "yes", "no"],
                  "default": "auto",
                  "description": "Whether the first non-empty row is a header. 'auto' (default) skips it when its count or population field is not numeric; 'yes' always skips it; 'no' treats every row as data."
                },
                "per": {
                  "type": "string",
                  "enum": ["1", "1000", "10000", "100000", "1000000", "custom"],
                  "default": "100000",
                  "description": "Reporting base the rate is scaled to: '1' = per person (per capita), '1000' = per 1,000, '10000' = per 10,000, '100000' = per 100,000 (default, the public-health convention), '1000000' = per 1,000,000, or 'custom' to use the custom_per value."
                },
                "custom_per": {
                  "type": "number",
                  "default": 0.0,
                  "minimum": 0,
                  "maximum": 1000000000000,
                  "description": "Custom reporting base, used only when per='custom' — for example 500 for 'per 500 residents'. Must be greater than 0 in that case. Ignored otherwise. Default 0."
                },
                "population_unit": {
                  "type": "string",
                  "enum": ["ones", "thousands", "millions"],
                  "default": "ones",
                  "description": "The unit the population column is expressed in. 'ones' (default) = actual people; 'thousands' multiplies it by 1,000 (World-Bank-style tables); 'millions' multiplies it by 1,000,000."
                },
                "decimals": {
                  "type": "integer",
                  "default": 2,
                  "minimum": 0,
                  "maximum": 6,
                  "description": "Decimal places for the rate and the overall rate, from 0 to 6. Default 2."
                },
                "sort": {
                  "type": "string",
                  "enum": ["rate_desc", "rate_asc", "input"],
                  "default": "rate_desc",
                  "description": "Row order in the report: 'rate_desc' (default) highest rate first, 'rate_asc' lowest first, or 'input' to keep the pasted order. Ties are broken by label."
                },
                "unstable_below": {
                  "type": "integer",
                  "default": 20,
                  "minimum": 0,
                  "maximum": 1000000,
                  "description": "Flag rows whose raw count is below this number as 'unstable', the convention that rates built on very few events are unreliable. Default 20; set 0 to flag nothing (every row then reads 'ok')."
                },
                "output": {
                  "type": "string",
                  "enum": ["table", "csv", "markdown", "json"],
                  "default": "table",
                  "description": "Output format: 'table' (default) tab-separated columns plus a text rate chart, 'csv' a spreadsheet-ready file with a metric header block, 'markdown' a report table, or 'json' structured rows."
                }
              }
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }
}
