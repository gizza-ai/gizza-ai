//! gizza-ai/frequency-encoder — frequency (count) encode a CSV categorical column.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn mode_default() -> String {
    "count".to_string()
}
fn output_default() -> String {
    "replace".to_string()
}
fn blank_default() -> String {
    "count".to_string()
}
fn decimals_default() -> usize {
    4
}
fn case_sensitive_default() -> bool {
    true
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
    column: String,
    #[serde(default = "mode_default")]
    mode: String,
    #[serde(default = "output_default")]
    output: String,
    #[serde(default = "blank_default")]
    blank: String,
    #[serde(default)]
    min_count: usize,
    #[serde(default = "case_sensitive_default")]
    case_sensitive: bool,
    #[serde(default = "decimals_default")]
    decimals: usize,
    #[serde(default = "has_header_default")]
    has_header: bool,
    #[serde(default = "delimiter_default")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("CSV text to encode. The chosen column is replaced (or a new column appended) with how often each value occurs."))
        .param(Param::string("column").required().describe("Categorical column to encode: a header name, or a 1-based column number when there is no header. Example: product_id."))
        .param(Param::enumv("mode", ["count", "frequency", "percent", "log-count"]).default("count").describe("What each value becomes: 'count' = raw number of rows with that value; 'frequency' = share of rows (0-1); 'percent' = share x 100; 'log-count' = ln(1 + count), which compresses very skewed distributions."))
        .param(Param::enumv("output", ["replace", "append"]).default("replace").describe("Replace the column in place, or keep it and append a new <name>_count / _freq / _pct / _logcount column."))
        .param(Param::enumv("blank", ["count", "nan", "zero"]).default("count").describe("How to treat blank cells: 'count' makes blanks their own category, 'nan' writes NaN, 'zero' writes 0. With nan/zero, blank rows are excluded from the counts and from the frequency denominator."))
        .param(Param::integer("min_count").default(0).min(0.0).max(100000.0).describe("Pool rare categories: values occurring fewer than this many times share one combined count, so rare levels collapse into a single group. 0 or 1 disables pooling."))
        .param(Param::boolean("case_sensitive").default(true).describe("Count values differing only in case separately. Turn off to group 'Paris', 'PARIS', and 'paris' as one value."))
        .param(Param::integer("decimals").default(4).min(0.0).max(15.0).describe("Number of decimal places for frequency, percent, and log-count values. Raw counts are always whole numbers."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as headers. Turn off to select the column by 1-based number."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV delimiter used to read and write the data."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/frequency-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Frequency-encode a CSV categorical column by value counts",
    skill(
        description = "Replace a categorical CSV column with how often each value occurs, turning a high-cardinality column into a single numeric feature without one-hot column explosion (count / frequency encoding). Modes: raw count, frequency share (0-1), percent, or log-count. Supports replace-in-place or append-new-column output, rare-category pooling below a minimum count, case-insensitive grouping, blank-cell handling (count / NaN / zero), decimal rounding, header or 1-based index column selection, and comma/tab/semicolon/pipe delimiters.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "frequency-encoder", |a: Args| {
            gizza_ai_frequency_encoder_core::encode(
                &a.data,
                &a.column,
                &a.mode,
                &a.output,
                &a.blank,
                a.min_count,
                a.case_sensitive,
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
                "data":{"type":"string","description":"CSV text to encode. The chosen column is replaced (or a new column appended) with how often each value occurs."},
                "column":{"type":"string","description":"Categorical column to encode: a header name, or a 1-based column number when there is no header. Example: product_id."},
                "mode":{"type":"string","enum":["count","frequency","percent","log-count"],"default":"count","description":"What each value becomes: 'count' = raw number of rows with that value; 'frequency' = share of rows (0-1); 'percent' = share x 100; 'log-count' = ln(1 + count), which compresses very skewed distributions."},
                "output":{"type":"string","enum":["replace","append"],"default":"replace","description":"Replace the column in place, or keep it and append a new <name>_count / _freq / _pct / _logcount column."},
                "blank":{"type":"string","enum":["count","nan","zero"],"default":"count","description":"How to treat blank cells: 'count' makes blanks their own category, 'nan' writes NaN, 'zero' writes 0. With nan/zero, blank rows are excluded from the counts and from the frequency denominator."},
                "min_count":{"type":"integer","minimum":0,"maximum":100000,"default":0,"description":"Pool rare categories: values occurring fewer than this many times share one combined count, so rare levels collapse into a single group. 0 or 1 disables pooling."},
                "case_sensitive":{"type":"boolean","default":true,"description":"Count values differing only in case separately. Turn off to group 'Paris', 'PARIS', and 'paris' as one value."},
                "decimals":{"type":"integer","minimum":0,"maximum":15,"default":4,"description":"Number of decimal places for frequency, percent, and log-count values. Raw counts are always whole numbers."},
                "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as headers. Turn off to select the column by 1-based number."},
                "delimiter":{"type":"string","enum":["comma","tab","semicolon","pipe"],"default":"comma","description":"CSV delimiter used to read and write the data."}
            },
            "required":["data","column"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
