//! gizza-ai/one-hot-encoder — expand a CSV categorical column into binary indicator columns.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn separator_default() -> String {
    "_".to_string()
}
fn drop_default() -> String {
    "none".to_string()
}
fn missing_default() -> String {
    "zeros".to_string()
}
fn sort_default() -> String {
    "alphabetical".to_string()
}
fn positive_default() -> String {
    "1".to_string()
}
fn negative_default() -> String {
    "0".to_string()
}
fn drop_original_default() -> bool {
    true
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
    #[serde(default)]
    prefix: String,
    #[serde(default = "separator_default")]
    separator: String,
    #[serde(default = "drop_default")]
    drop: String,
    #[serde(default = "drop_original_default")]
    drop_original: bool,
    #[serde(default = "missing_default")]
    missing: String,
    #[serde(default)]
    max_categories: usize,
    #[serde(default)]
    min_count: usize,
    #[serde(default)]
    other_column: bool,
    #[serde(default = "positive_default")]
    positive: String,
    #[serde(default = "negative_default")]
    negative: String,
    #[serde(default = "case_sensitive_default")]
    case_sensitive: bool,
    #[serde(default = "sort_default")]
    sort: String,
    #[serde(default = "has_header_default")]
    has_header: bool,
    #[serde(default = "delimiter_default")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("CSV text to encode. The chosen column is expanded into one binary indicator (dummy) column per distinct value."))
        .param(Param::string("column").required().describe("Categorical column to expand: a header name, or a 1-based column number when there is no header. Example: city."))
        .param(Param::string("prefix").describe("Prefix for the generated column names. Leave blank to use the source column's own name, so 'city' produces city_Paris, city_Rome."))
        .param(Param::string("separator").default("_").describe("Text placed between the prefix and the category value. Default '_' gives city_Paris; use '=' for city=Paris."))
        .param(Param::enumv("drop", ["none", "first", "last", "if-binary"]).default("none").describe("Drop one category as the reference level to avoid the dummy-variable trap (perfect collinearity in linear models): 'none' keeps all k columns, 'first'/'last' keep k-1, 'if-binary' drops the first level only when the column has exactly two categories."))
        .param(Param::boolean("drop_original").default(true).describe("Remove the original categorical column from the output, keeping only the indicator columns. Turn off to keep it alongside them."))
        .param(Param::enumv("missing", ["zeros", "separate", "blank", "error"]).default("zeros").describe("What a blank cell means: 'zeros' writes 0 in every indicator, 'separate' adds its own <prefix>_NaN indicator column, 'blank' leaves the indicator cells empty, 'error' rejects the input instead of guessing."))
        .param(Param::integer("max_categories").default(0).min(0.0).max(512.0).describe("Keep only the N most frequent categories and give the rest no column of their own. 0 keeps every category. Use this on high-cardinality columns to stop the output exploding."))
        .param(Param::integer("min_count").default(0).min(0.0).max(100000.0).describe("Keep only categories occurring at least this many times, so one-off values do not each get a column. 0 or 1 keeps every category."))
        .param(Param::boolean("other_column").default(false).describe("Add one combined <prefix>_other indicator for the categories excluded by max_categories or min_count. Off means those rows are 0 in every column."))
        .param(Param::string("positive").default("1").describe("Text written when a row belongs to that category. Default '1'; use 'true' or 'Y' for a boolean-style output."))
        .param(Param::string("negative").default("0").describe("Text written when a row does not belong to that category. Default '0'; use 'false' or 'N' to match the positive value's style."))
        .param(Param::boolean("case_sensitive").default(true).describe("Give values differing only in case their own columns. Turn off to fold 'Paris', 'PARIS', and 'paris' into one column named after the first spelling seen."))
        .param(Param::enumv("sort", ["alphabetical", "frequency", "first-seen"]).default("alphabetical").describe("Order of the generated columns: 'alphabetical' by value, 'frequency' most common first, or 'first-seen' in the order values appear in the data. max_categories always selects by frequency regardless of this."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as headers. Turn off to select the column by 1-based number; no header row is then written."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV delimiter used to read and write the data."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/one-hot-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "One-hot encode a CSV categorical column into binary dummy columns",
    skill(
        description = "Expand a categorical CSV column into one binary indicator column per distinct category (one-hot / dummy-variable encoding, as produced by pandas get_dummies or scikit-learn OneHotEncoder). Each row gets a 1 in the column matching its value and 0 elsewhere. Supports a custom column prefix and separator, dropping a reference level (first / last / only-if-binary) to avoid the dummy-variable trap, keeping or removing the source column, capping the expansion to the top-N most frequent categories or to values seen at least N times with an optional combined 'other' column, custom positive/negative values (1/0, true/false), blank-cell handling (zeros / separate NaN column / blank / error), case-insensitive grouping, alphabetical / frequency / first-seen column ordering, header or 1-based index column selection, and comma/tab/semicolon/pipe delimiters.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "one-hot-encoder", |a: Args| {
            gizza_ai_one_hot_encoder_core::encode(
                &a.data,
                &a.column,
                &a.prefix,
                &a.separator,
                &a.drop,
                a.drop_original,
                &a.missing,
                a.max_categories,
                a.min_count,
                a.other_column,
                &a.positive,
                &a.negative,
                a.case_sensitive,
                &a.sort,
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
                "data":{"type":"string","description":"CSV text to encode. The chosen column is expanded into one binary indicator (dummy) column per distinct value."},
                "column":{"type":"string","description":"Categorical column to expand: a header name, or a 1-based column number when there is no header. Example: city."},
                "prefix":{"type":"string","description":"Prefix for the generated column names. Leave blank to use the source column's own name, so 'city' produces city_Paris, city_Rome."},
                "separator":{"type":"string","default":"_","description":"Text placed between the prefix and the category value. Default '_' gives city_Paris; use '=' for city=Paris."},
                "drop":{"type":"string","enum":["none","first","last","if-binary"],"default":"none","description":"Drop one category as the reference level to avoid the dummy-variable trap (perfect collinearity in linear models): 'none' keeps all k columns, 'first'/'last' keep k-1, 'if-binary' drops the first level only when the column has exactly two categories."},
                "drop_original":{"type":"boolean","default":true,"description":"Remove the original categorical column from the output, keeping only the indicator columns. Turn off to keep it alongside them."},
                "missing":{"type":"string","enum":["zeros","separate","blank","error"],"default":"zeros","description":"What a blank cell means: 'zeros' writes 0 in every indicator, 'separate' adds its own <prefix>_NaN indicator column, 'blank' leaves the indicator cells empty, 'error' rejects the input instead of guessing."},
                "max_categories":{"type":"integer","minimum":0,"maximum":512,"default":0,"description":"Keep only the N most frequent categories and give the rest no column of their own. 0 keeps every category. Use this on high-cardinality columns to stop the output exploding."},
                "min_count":{"type":"integer","minimum":0,"maximum":100000,"default":0,"description":"Keep only categories occurring at least this many times, so one-off values do not each get a column. 0 or 1 keeps every category."},
                "other_column":{"type":"boolean","default":false,"description":"Add one combined <prefix>_other indicator for the categories excluded by max_categories or min_count. Off means those rows are 0 in every column."},
                "positive":{"type":"string","default":"1","description":"Text written when a row belongs to that category. Default '1'; use 'true' or 'Y' for a boolean-style output."},
                "negative":{"type":"string","default":"0","description":"Text written when a row does not belong to that category. Default '0'; use 'false' or 'N' to match the positive value's style."},
                "case_sensitive":{"type":"boolean","default":true,"description":"Give values differing only in case their own columns. Turn off to fold 'Paris', 'PARIS', and 'paris' into one column named after the first spelling seen."},
                "sort":{"type":"string","enum":["alphabetical","frequency","first-seen"],"default":"alphabetical","description":"Order of the generated columns: 'alphabetical' by value, 'frequency' most common first, or 'first-seen' in the order values appear in the data. max_categories always selects by frequency regardless of this."},
                "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as headers. Turn off to select the column by 1-based number; no header row is then written."},
                "delimiter":{"type":"string","enum":["comma","tab","semicolon","pipe"],"default":"comma","description":"CSV delimiter used to read and write the data."}
            },
            "required":["data","column"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
