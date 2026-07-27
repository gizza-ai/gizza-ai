//! gizza-ai/person-name-splitter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute; the parsing
//! heuristics live in core::run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_person_name_splitter_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    name_column: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    trim: bool,
}
fn default_output() -> String {
    "append".into()
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text whose name column to split. Each row's name cell is parsed into title, first, middle, last, and suffix; every other cell is kept verbatim."))
        .param(Param::string("name_column").default("").describe("Which column holds the full name: a header name (when header=true) or a 1-based column number. Blank = the first column. Default blank (first column)."))
        .param(Param::enumv("output", ["append", "replace", "summary"]).default("append").describe("What to return: append (keep every original column and add the five component columns at the end), replace (swap the name column in place for the five component columns), or summary (a JSON report of component counts and the rows that could not be split cleanly). Default append."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header — kept in the output, used to resolve name_column names, and the source of the `<name>_title/_first/_middle/_last/_suffix` column names. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter of the CSV, used for both reading the input and writing the output. Default comma."))
        .param(Param::boolean("trim").default(true).describe("Trim surrounding whitespace from every cell before parsing. When false, cells keep their original padding (only the name being parsed is always trimmed). Default true."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/person-name-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a CSV column of full names into title/first/middle/last/suffix",
    skill(
        description = "Split a CSV column of full person names into structured title, first, middle, last, and suffix columns using order-independent heuristics. Understands honorific titles (Mr, Ms, Dr, Prof), generational/credential suffixes (Jr, Sr, II, III, PhD, MD), surname particles (van, von, de, del, di, la, mac, mc), hyphenated and apostrophe names, and the `Last, First Middle` comma form. `name_column` is a header name or 1-based index (blank = first column). `output` is append (add the five columns), replace (swap the name column for them), or summary (a JSON count report listing rows that could not be split cleanly). Runs entirely locally; no network, no AI guessing.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "person-name-splitter", |a: Args| {
            run(
                &a.data,
                &a.name_column,
                &a.output,
                a.header,
                &a.delimiter,
                a.trim,
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
                    "data":        { "type": "string", "description": "The CSV text whose name column to split. Each row's name cell is parsed into title, first, middle, last, and suffix; every other cell is kept verbatim." },
                    "name_column": { "type": "string", "default": "", "description": "Which column holds the full name: a header name (when header=true) or a 1-based column number. Blank = the first column. Default blank (first column)." },
                    "output":      { "type": "string", "enum": ["append", "replace", "summary"], "default": "append", "description": "What to return: append (keep every original column and add the five component columns at the end), replace (swap the name column in place for the five component columns), or summary (a JSON report of component counts and the rows that could not be split cleanly). Default append." },
                    "header":      { "type": "boolean", "default": true, "description": "Treat the first row as a header — kept in the output, used to resolve name_column names, and the source of the `<name>_title/_first/_middle/_last/_suffix` column names. Default true." },
                    "delimiter":   { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter of the CSV, used for both reading the input and writing the output. Default comma." },
                    "trim":        { "type": "boolean", "default": true, "description": "Trim surrounding whitespace from every cell before parsing. When false, cells keep their original padding (only the name being parsed is always trimmed). Default true." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
