//! gizza-ai/regex-bulk-match — test one regex against many lines, one verdict per line.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn trim_default() -> bool {
    true
}
fn skip_blank_default() -> bool {
    true
}
fn captures_default() -> bool {
    true
}
fn show_default() -> String {
    "all".to_string()
}
fn max_lines_default() -> usize {
    1000
}
fn output_default() -> String {
    "text".to_string()
}

#[derive(Deserialize)]
struct Args {
    lines: String,
    pattern: String,
    #[serde(default)]
    full_match: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default = "trim_default")]
    trim: bool,
    #[serde(default = "skip_blank_default")]
    skip_blank: bool,
    #[serde(default = "captures_default")]
    captures: bool,
    #[serde(default)]
    show_position: bool,
    #[serde(default = "show_default")]
    show: String,
    #[serde(default = "max_lines_default")]
    max_lines: usize,
    #[serde(default = "output_default")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("lines").required().multiline().describe("Input text: one test string per line, for example a pasted list of emails, IDs, or log lines."))
        .param(Param::string("pattern").required().describe("Rust regular expression to test against each line, for example \\d{5} or ^[\\w.+-]+@([\\w-]+\\.[\\w.]+)$. Inline flags like (?i), (?m), (?s), and (?x) are supported."))
        .param(Param::boolean("full_match").default(false).describe("Require the pattern to match the whole line. Off (default) reports a match when the pattern is found anywhere in the line."))
        .param(Param::boolean("ignore_case").default(false).describe("Match case-insensitively, the same as prefixing the pattern with (?i)."))
        .param(Param::boolean("dotall").default(false).describe("Let . also match a newline, the same as (?s). Only matters for patterns that span line breaks."))
        .param(Param::boolean("trim").default(true).describe("Strip leading and trailing whitespace from each line before testing. Turn off to test lines exactly as pasted."))
        .param(Param::boolean("skip_blank").default(true).describe("Ignore blank lines instead of counting them as failures. Line numbers still refer to the original input."))
        .param(Param::boolean("captures").default(true).describe("Report capture groups for each matching line. Named groups like (?<name>...) are reported by name, unnamed groups by number."))
        .param(Param::boolean("show_position").default(false).describe("Add the match start and end byte offsets to the text and CSV reports. JSON output always includes them."))
        .param(Param::enumv("show", ["all", "matching", "non-matching"]).default("all").describe("Which lines to list: every line with its verdict, only the lines that matched, or only the lines that did not. Totals always cover every tested line."))
        .param(Param::integer("max_lines").default(1000).min(1.0).max(20000.0).describe("Maximum number of lines to test. Extra lines are skipped and the report is flagged as truncated."))
        .param(Param::enumv("output", ["text", "json", "csv"]).default("text").describe("Output format: a readable per-line report, a JSON object, or CSV with one column per capture group."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-bulk-match",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Test one regex against many lines and report match, no-match, and captures per line",
    skill(
        description = "Test a single regular expression against many input lines at once and report a verdict for every line: matched or not matched, the matched text, its offsets, and each capture group (named groups by name). Supports whole-line or match-anywhere mode, case-insensitive and dot-all matching, whitespace trimming, blank-line skipping, filtering to only matching or only non-matching lines, a line cap, and text, JSON, or CSV output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-bulk-match", |a: Args| {
            gizza_ai_regex_bulk_match_core::run(
                &a.lines,
                &a.pattern,
                a.full_match,
                a.ignore_case,
                a.dotall,
                a.trim,
                a.skip_blank,
                a.captures,
                a.show_position,
                &a.show,
                a.max_lines,
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
                "lines":{"type":"string","description":"Input text: one test string per line, for example a pasted list of emails, IDs, or log lines."},
                "pattern":{"type":"string","description":"Rust regular expression to test against each line, for example \\d{5} or ^[\\w.+-]+@([\\w-]+\\.[\\w.]+)$. Inline flags like (?i), (?m), (?s), and (?x) are supported."},
                "full_match":{"type":"boolean","default":false,"description":"Require the pattern to match the whole line. Off (default) reports a match when the pattern is found anywhere in the line."},
                "ignore_case":{"type":"boolean","default":false,"description":"Match case-insensitively, the same as prefixing the pattern with (?i)."},
                "dotall":{"type":"boolean","default":false,"description":"Let . also match a newline, the same as (?s). Only matters for patterns that span line breaks."},
                "trim":{"type":"boolean","default":true,"description":"Strip leading and trailing whitespace from each line before testing. Turn off to test lines exactly as pasted."},
                "skip_blank":{"type":"boolean","default":true,"description":"Ignore blank lines instead of counting them as failures. Line numbers still refer to the original input."},
                "captures":{"type":"boolean","default":true,"description":"Report capture groups for each matching line. Named groups like (?<name>...) are reported by name, unnamed groups by number."},
                "show_position":{"type":"boolean","default":false,"description":"Add the match start and end byte offsets to the text and CSV reports. JSON output always includes them."},
                "show":{"type":"string","enum":["all","matching","non-matching"],"default":"all","description":"Which lines to list: every line with its verdict, only the lines that matched, or only the lines that did not. Totals always cover every tested line."},
                "max_lines":{"type":"integer","minimum":1,"maximum":20000,"default":1000,"description":"Maximum number of lines to test. Extra lines are skipped and the report is flagged as truncated."},
                "output":{"type":"string","enum":["text","json","csv"],"default":"text","description":"Output format: a readable per-line report, a JSON object, or CSV with one column per capture group."}
            },
            "required":["lines","pattern"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
