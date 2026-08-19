//! gizza-ai/regex-capture-to-csv — scan text with a regex and emit one CSV row
//! per match, capture groups as columns. Chat schema single-sourced from
//! descriptor(); handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_capture_to_csv_core::to_csv;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_quoting")]
    quoting: String,
    #[serde(default = "default_line_ending")]
    line_ending: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    sort: bool,
}

fn default_delimiter() -> String {
    ",".to_string()
}
fn default_true() -> bool {
    true
}
fn default_quoting() -> String {
    "minimal".to_string()
}
fn default_line_ending() -> String {
    "lf".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan — logs, HTML, exports, command output. The regex is applied to the whole text, so a match may span several lines. Max 1 MB."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("Regular expression (Rust regex syntax). Columns come from its capture groups: NAMED groups — (?<name>…) or (?P<name>…) — become the header, e.g. (?<ip>\\S+) (?<status>\\d{3}). A pattern with only unnamed groups gets column1, column2, …; a pattern with no groups gets a single 'match' column holding the whole match."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Comma-separated column names to emit, in the order you want them (e.g. 'status, ip'). Blank (default) emits every capture group in pattern order. A name may repeat; an unknown name is an error that lists the available names."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field delimiter: a single character, the escape \\t, or one of the keywords comma, semicolon, tab, pipe, colon, space. Default ','."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit a first row of column names. Default true; turn it off to append the rows to an existing CSV."),
        )
        .param(
            Param::enumv("quoting", ["minimal", "all"])
                .default("minimal")
                .describe("When to wrap a field in double quotes: 'minimal' (default) quotes only fields containing the delimiter, a quote, or a line break; 'all' quotes every field including the header. Embedded quotes are always doubled (RFC 4180)."),
        )
        .param(
            Param::enumv("line_ending", ["lf", "crlf"])
                .default("lf")
                .describe("Row terminator: 'lf' (default, Unix) or 'crlf' (Windows/Excel-friendly)."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively (the i flag)."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe("Let ^ and $ match at line boundaries, not only the start/end of the text (the m flag)."),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("Let . also match newline characters (the s flag) so one match can span lines — useful for HTML blocks and stack traces."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("Drop duplicate rows, keeping first-seen order."),
        )
        .param(
            Param::boolean("sort")
                .default(false)
                .describe("Sort the rows lexicographically by the first column, then the rest. Applied after unique."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-capture-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn regex capture groups into CSV rows — one row per match",
    skill(
        description = "Scan text with a regular expression (Rust regex syntax) and emit CSV: one row per match, with the capture groups as columns. Named groups ((?<name>…) or (?P<name>…)) supply the header; unnamed groups become column1, column2, …; a pattern with no groups yields a single 'match' column. Pick and reorder columns, choose the delimiter (comma, semicolon, tab, pipe, colon, space or any character), toggle the header row, quote minimally or always (RFC 4180 doubling), pick LF or CRLF line endings, apply the i/m/s regex flags, and optionally dedupe or sort rows. The pattern runs against the whole text, so with dotall a match may span lines. Groups that did not participate yield empty fields. Input is capped at 1 MB and 100000 rows. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-capture-to-csv", |a: Args| {
            to_csv(
                &a.text,
                &a.pattern,
                &a.columns,
                &a.delimiter,
                a.header,
                &a.quoting,
                &a.line_ending,
                a.ignore_case,
                a.multiline,
                a.dotall,
                a.unique,
                a.sort,
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
                    "text": { "type": "string", "description": "The text to scan — logs, HTML, exports, command output. The regex is applied to the whole text, so a match may span several lines. Max 1 MB." },
                    "pattern": { "type": "string", "description": "Regular expression (Rust regex syntax). Columns come from its capture groups: NAMED groups — (?<name>…) or (?P<name>…) — become the header, e.g. (?<ip>\\S+) (?<status>\\d{3}). A pattern with only unnamed groups gets column1, column2, …; a pattern with no groups gets a single 'match' column holding the whole match." },
                    "columns": { "type": "string", "default": "", "description": "Comma-separated column names to emit, in the order you want them (e.g. 'status, ip'). Blank (default) emits every capture group in pattern order. A name may repeat; an unknown name is an error that lists the available names." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field delimiter: a single character, the escape \\t, or one of the keywords comma, semicolon, tab, pipe, colon, space. Default ','." },
                    "header": { "type": "boolean", "default": true, "description": "Emit a first row of column names. Default true; turn it off to append the rows to an existing CSV." },
                    "quoting": { "type": "string", "enum": ["minimal", "all"], "default": "minimal", "description": "When to wrap a field in double quotes: 'minimal' (default) quotes only fields containing the delimiter, a quote, or a line break; 'all' quotes every field including the header. Embedded quotes are always doubled (RFC 4180)." },
                    "line_ending": { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Row terminator: 'lf' (default, Unix) or 'crlf' (Windows/Excel-friendly)." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively (the i flag)." },
                    "multiline": { "type": "boolean", "default": false, "description": "Let ^ and $ match at line boundaries, not only the start/end of the text (the m flag)." },
                    "dotall": { "type": "boolean", "default": false, "description": "Let . also match newline characters (the s flag) so one match can span lines — useful for HTML blocks and stack traces." },
                    "unique": { "type": "boolean", "default": false, "description": "Drop duplicate rows, keeping first-seen order." },
                    "sort": { "type": "boolean", "default": false, "description": "Sort the rows lexicographically by the first column, then the rest. Applied after unique." }
                },
                "required": ["text", "pattern"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_apply() {
        let a: Args =
            serde_json::from_str(r#"{"text":"alice 30","pattern":"(?<name>[a-z]+) (?<age>\\d+)"}"#)
                .unwrap();
        assert_eq!(a.columns, "");
        assert_eq!(a.delimiter, ",");
        assert!(a.header);
        assert_eq!(a.quoting, "minimal");
        assert_eq!(a.line_ending, "lf");
        assert!(!a.ignore_case && !a.multiline && !a.dotall && !a.unique && !a.sort);
        let out = to_csv(
            &a.text,
            &a.pattern,
            &a.columns,
            &a.delimiter,
            a.header,
            &a.quoting,
            &a.line_ending,
            a.ignore_case,
            a.multiline,
            a.dotall,
            a.unique,
            a.sort,
        )
        .unwrap();
        assert_eq!(out, "name,age\nalice,30");
    }
}
