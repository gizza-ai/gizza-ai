//! gizza-ai/target-mean-encoder — target (mean) encode a CSV categorical column.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn smoothing_default() -> f64 {
    0.0
}
fn output_default() -> String {
    "replace".to_string()
}
fn unknown_default() -> String {
    "global-mean".to_string()
}
fn decimals_default() -> usize {
    4
}
fn has_header_default() -> bool {
    true
}
fn delimiter_default() -> String {
    "comma".to_string()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    category: String,
    target: String,
    #[serde(default = "smoothing_default")]
    smoothing: f64,
    #[serde(default)]
    leave_one_out: bool,
    #[serde(default = "output_default")]
    output: String,
    #[serde(default = "unknown_default")]
    unknown: String,
    #[serde(default = "decimals_default")]
    decimals: usize,
    #[serde(default = "has_header_default")]
    has_header: bool,
    #[serde(default = "delimiter_default")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("CSV text to encode. The category column is replaced (or a new column appended) with the target mean per category."))
        .param(Param::string("category").required().describe("Categorical column to encode: a header name, or a 1-based column number when there is no header."))
        .param(Param::string("target").required().describe("Numeric target column whose per-category mean drives the encoding: a header name, or a 1-based column number."))
        .param(Param::number("smoothing").default(0.0).min(0.0).max(100.0).describe("m-estimate smoothing weight (m ≥ 0). Blends each category mean toward the global mean as (sum + m·prior) / (count + m). 0 means the raw category mean; higher values pull rare categories toward the overall mean."))
        .param(Param::boolean("leave_one_out").default(false).describe("Exclude each row's own target from its category statistics. Reduces target leakage when encoding a training set in place."))
        .param(Param::enumv("output", ["replace", "append"]).default("replace").describe("Replace the category column in place, or append a new <name>_target_enc column and keep the original."))
        .param(Param::enumv("unknown", ["global-mean", "nan", "zero"]).default("global-mean").describe("Value for rows whose category cell is blank or has no usable statistics: the global mean, NaN, or zero."))
        .param(Param::integer("decimals").default(4).min(0.0).max(15.0).describe("Number of decimal places for the encoded values."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as headers. Turn off to select columns by 1-based number."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV delimiter used to read and write the data."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/target-mean-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Target-encode a CSV categorical column by target mean",
    skill(
        description = "Replace a categorical CSV column with the mean of a numeric target per category, turning it into a single model-ready numeric feature (target / mean / impact encoding). Supports m-estimate smoothing toward the global mean for rare categories, leave-one-out to reduce target leakage, replace-in-place or append-new-column output, configurable handling of unknown/blank categories, decimal rounding, header or 1-based index column selection, and comma/tab/semicolon/pipe delimiters.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "target-mean-encoder", |a: Args| {
            gizza_ai_target_mean_encoder_core::encode(
                &a.data,
                &a.category,
                &a.target,
                a.smoothing,
                a.leave_one_out,
                &a.output,
                &a.unknown,
                a.decimals,
                a.has_header,
                &a.delimiter,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "data":{"type":"string","description":"CSV text to encode. The category column is replaced (or a new column appended) with the target mean per category."},
                "category":{"type":"string","description":"Categorical column to encode: a header name, or a 1-based column number when there is no header."},
                "target":{"type":"string","description":"Numeric target column whose per-category mean drives the encoding: a header name, or a 1-based column number."},
                "smoothing":{"type":"number","minimum":0,"maximum":100,"default":0.0,"description":"m-estimate smoothing weight (m ≥ 0). Blends each category mean toward the global mean as (sum + m·prior) / (count + m). 0 means the raw category mean; higher values pull rare categories toward the overall mean."},
                "leave_one_out":{"type":"boolean","default":false,"description":"Exclude each row's own target from its category statistics. Reduces target leakage when encoding a training set in place."},
                "output":{"type":"string","enum":["replace","append"],"default":"replace","description":"Replace the category column in place, or append a new <name>_target_enc column and keep the original."},
                "unknown":{"type":"string","enum":["global-mean","nan","zero"],"default":"global-mean","description":"Value for rows whose category cell is blank or has no usable statistics: the global mean, NaN, or zero."},
                "decimals":{"type":"integer","minimum":0,"maximum":15,"default":4,"description":"Number of decimal places for the encoded values."},
                "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as headers. Turn off to select columns by 1-based number."},
                "delimiter":{"type":"string","enum":["comma","tab","semicolon","pipe"],"default":"comma","description":"CSV delimiter used to read and write the data."}
            },
            "required":["data","category","target"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
