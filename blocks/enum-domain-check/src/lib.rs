//! gizza-ai/enum-domain-check — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn ignore_case_default() -> bool {
    false
}
fn trim_default() -> bool {
    true
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
    allowed: String,
    #[serde(default = "ignore_case_default")]
    ignore_case: bool,
    #[serde(default = "trim_default")]
    trim: bool,
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

/// Single source for the chat schema (and CLI). Flags CSV column cells whose value
/// is not one of an allowed set of category values.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("CSV text to validate."))
        .param(Param::string("column").required().describe("Target column header name, or a 0-based column index. Numeric values are always treated as indexes."))
        .param(Param::string("allowed").required().describe("The allowed set of category values, comma-separated (e.g. active,inactive,pending). Surrounding spaces are trimmed and duplicates ignored."))
        .param(Param::boolean("ignore_case").default(false).describe("Compare values case-insensitively when checking membership."))
        .param(Param::boolean("trim").default(true).describe("Trim surrounding whitespace from each cell before comparing. Turn off to require an exact, unpadded match."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as headers. Turn off to use 0-based numeric column indexes."))
        .param(Param::boolean("allow_blank").default(true).describe("Treat blank cells as valid without checking membership. Turn off to flag blank cells."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe("CSV delimiter. Auto detects comma, tab, semicolon, or pipe from the first non-blank line."))
        .param(Param::integer("max_issues").default(50).min(1.0).max(1000.0).describe("Maximum number of offending rows (and distinct unexpected values) to list; total invalid count is still reported."))
        .param(Param::enumv("output", ["text", "json"]).default("text").describe("Output format: readable text report or structured JSON."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/enum-domain-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag CSV column values outside an allowed set of categories",
    skill(
        description = "Check one CSV column against an allowed set of category values (a domain / controlled-vocabulary / allowed-values check). Flags every cell whose value is not one of the allowed categories, and summarizes the distinct unexpected values with counts so typos are easy to spot. Supports header or index column selection, delimiter auto-detection, case-insensitive matching, whitespace trimming, blank-cell policy, capped issue lists, and text or JSON reports.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "enum-domain-check", |a: Args| {
            gizza_ai_enum_domain_check_core::run(
                &a.data,
                &a.column,
                &a.allowed,
                a.ignore_case,
                a.trim,
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
                "allowed":{"type":"string","description":"The allowed set of category values, comma-separated (e.g. active,inactive,pending). Surrounding spaces are trimmed and duplicates ignored."},
                "ignore_case":{"type":"boolean","default":false,"description":"Compare values case-insensitively when checking membership."},
                "trim":{"type":"boolean","default":true,"description":"Trim surrounding whitespace from each cell before comparing. Turn off to require an exact, unpadded match."},
                "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as headers. Turn off to use 0-based numeric column indexes."},
                "allow_blank":{"type":"boolean","default":true,"description":"Treat blank cells as valid without checking membership. Turn off to flag blank cells."},
                "delimiter":{"type":"string","enum":["auto","comma","tab","semicolon","pipe"],"default":"auto","description":"CSV delimiter. Auto detects comma, tab, semicolon, or pipe from the first non-blank line."},
                "max_issues":{"type":"integer","minimum":1,"maximum":1000,"default":50,"description":"Maximum number of offending rows (and distinct unexpected values) to list; total invalid count is still reported."},
                "output":{"type":"string","enum":["text","json"],"default":"text","description":"Output format: readable text report or structured JSON."}
            },
            "required":["data","column","allowed"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
