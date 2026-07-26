//! gizza-ai/regex-column-validate — validate a CSV column with a regex.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn full_match_default() -> bool {
    true
}
fn report_default() -> String {
    "non-matching".to_string()
}
fn has_header_default() -> bool {
    true
}
fn allow_blank_default() -> bool {
    true
}
fn delimiter_default() -> String {
    "auto".to_string()
}
fn max_issues_default() -> usize {
    50
}
fn output_default() -> String {
    "text".to_string()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    column: String,
    pattern: String,
    #[serde(default = "full_match_default")]
    full_match: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "report_default")]
    report: String,
    #[serde(default = "has_header_default")]
    has_header: bool,
    #[serde(default = "allow_blank_default")]
    allow_blank: bool,
    #[serde(default = "delimiter_default")]
    delimiter: String,
    #[serde(default = "max_issues_default")]
    max_issues: usize,
    #[serde(default = "output_default")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("CSV text to validate."))
        .param(Param::string("column").required().describe("Target column header name, or a 0-based column index. Numeric values are always treated as indexes."))
        .param(Param::string("pattern").required().describe("Rust regular expression to test against each cell. Use inline flags like (?i), (?m), or (?s) for advanced modes."))
        .param(Param::boolean("full_match").default(true).describe("Require the pattern to match the whole cell. Turn off to accept a match anywhere inside the cell."))
        .param(Param::boolean("ignore_case").default(false).describe("Match case-insensitively."))
        .param(Param::enumv("report", ["non-matching", "matching"]).default("non-matching").describe("Which rows to flag: values that do not match, or values that do match (for forbidden-pattern checks)."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as headers. Turn off to use 0-based numeric column indexes."))
        .param(Param::boolean("allow_blank").default(true).describe("Treat blank cells as valid without testing the regex."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe("CSV delimiter. Auto detects comma, tab, semicolon, or pipe from the first non-blank line."))
        .param(Param::integer("max_issues").default(50).min(1.0).max(1000.0).describe("Maximum number of offending rows to list; total invalid count is still reported."))
        .param(Param::enumv("output", ["text", "json"]).default("text").describe("Output format: readable text report or structured JSON."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-column-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate CSV column values with a regex",
    skill(
        description = "Validate one CSV column against a regular expression. Flags non-matching values by default, or matching values for forbidden-pattern checks. Supports header or index column selection, delimiter auto-detection, full-cell versus match-anywhere mode, case-insensitive matching, blank-cell policy, capped issue lists, and text or JSON reports.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-column-validate", |a: Args| {
            gizza_ai_regex_column_validate_core::run(
                &a.data,
                &a.column,
                &a.pattern,
                a.full_match,
                a.ignore_case,
                &a.report,
                a.has_header,
                a.allow_blank,
                &a.delimiter,
                a.max_issues,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "data":{"type":"string","description":"CSV text to validate."},
                "column":{"type":"string","description":"Target column header name, or a 0-based column index. Numeric values are always treated as indexes."},
                "pattern":{"type":"string","description":"Rust regular expression to test against each cell. Use inline flags like (?i), (?m), or (?s) for advanced modes."},
                "full_match":{"type":"boolean","default":true,"description":"Require the pattern to match the whole cell. Turn off to accept a match anywhere inside the cell."},
                "ignore_case":{"type":"boolean","default":false,"description":"Match case-insensitively."},
                "report":{"type":"string","enum":["non-matching","matching"],"default":"non-matching","description":"Which rows to flag: values that do not match, or values that do match (for forbidden-pattern checks)."},
                "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as headers. Turn off to use 0-based numeric column indexes."},
                "allow_blank":{"type":"boolean","default":true,"description":"Treat blank cells as valid without testing the regex."},
                "delimiter":{"type":"string","enum":["auto","comma","tab","semicolon","pipe"],"default":"auto","description":"CSV delimiter. Auto detects comma, tab, semicolon, or pipe from the first non-blank line."},
                "max_issues":{"type":"integer","minimum":1,"maximum":1000,"default":50,"description":"Maximum number of offending rows to list; total invalid count is still reported."},
                "output":{"type":"string","enum":["text","json"],"default":"text","description":"Output format: readable text report or structured JSON."}
            },
            "required":["data","column","pattern"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
