//! gizza-ai/field-extractor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "fields".into()
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    selectors: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    output_delimiter: String,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    skip_empty_lines: bool,
    #[serde(default)]
    skip_header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().multiline().describe("Input text, one record per line."))
        .param(Param::enumv("mode", ["fields", "chars"]).default("fields").describe("What to extract: 'fields' (default) splits each line into columns by the delimiter; 'chars' extracts character positions from each line (Unicode code-point safe, like cut -c)."))
        .param(Param::string("selectors").required().describe("1-based selectors, comma-separated. A term is a single index (1, 3), a negative index counting from the end (-1 = last, -2 = second-to-last), a range (2-4), a reversed range (4-2), or an open-ended range (3- = from field 3 to the end). Endpoints may be negative (-3--1). Terms emit in the order given, so 3,1,2 reorders."))
        .param(Param::string("delimiter").default("").describe("Field delimiter for 'fields' mode. Blank (default) collapses runs of whitespace (like awk). Accepts multi-character strings, keyword names (tab, comma, pipe, semicolon, colon, space), and backslash escapes (\\t, \\n). Ignored in 'chars' mode."))
        .param(Param::string("output_delimiter").default("").describe("Delimiter used to join the extracted pieces. Blank (default) reuses the input delimiter in 'fields' mode (a single space when whitespace-splitting) and concatenates in 'chars' mode. Accepts keyword names and \\t/\\n escapes; 'newline' puts each piece on its own line."))
        .param(Param::boolean("trim").default(false).describe("Trim surrounding whitespace from each extracted field ('fields' mode). Default false."))
        .param(Param::boolean("skip_empty_lines").default(false).describe("Drop blank or whitespace-only input lines instead of emitting an empty output line. Default false."))
        .param(Param::boolean("skip_header").default(false).describe("Drop the first line of input (a header row) before extracting. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/field-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract fields (columns) or character ranges from each line of text — a cut/awk replacement with negative indices.",
    skill(
        description = "Extract specific fields or character ranges from every line of text, a friendly cut/awk replacement. In 'fields' mode split each line by a delimiter (blank = collapse whitespace) and pick columns with 1-based selectors that support negative indices (-1 = last field), ranges (2-4), reversed ranges (4-2), open-ended ranges (3-), and reordering (3,1,2). In 'chars' mode extract character positions (Unicode code-point safe). Options: custom multi-character/escaped delimiters, an output delimiter, trim, skip empty lines, and skip a header row. Returns the extracted text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "field-extractor", |a: Args| {
            gizza_ai_field_extractor_core::extract(
                &a.text,
                &a.mode,
                &a.selectors,
                &a.delimiter,
                &a.output_delimiter,
                a.trim,
                a.skip_empty_lines,
                a.skip_header,
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
          "type":"object","properties":{
            "text":{"type":"string","description":"Input text, one record per line."},
            "mode":{"type":"string","enum":["fields","chars"],"default":"fields","description":"What to extract: 'fields' (default) splits each line into columns by the delimiter; 'chars' extracts character positions from each line (Unicode code-point safe, like cut -c)."},
            "selectors":{"type":"string","description":"1-based selectors, comma-separated. A term is a single index (1, 3), a negative index counting from the end (-1 = last, -2 = second-to-last), a range (2-4), a reversed range (4-2), or an open-ended range (3- = from field 3 to the end). Endpoints may be negative (-3--1). Terms emit in the order given, so 3,1,2 reorders."},
            "delimiter":{"type":"string","default":"","description":"Field delimiter for 'fields' mode. Blank (default) collapses runs of whitespace (like awk). Accepts multi-character strings, keyword names (tab, comma, pipe, semicolon, colon, space), and backslash escapes (\\t, \\n). Ignored in 'chars' mode."},
            "output_delimiter":{"type":"string","default":"","description":"Delimiter used to join the extracted pieces. Blank (default) reuses the input delimiter in 'fields' mode (a single space when whitespace-splitting) and concatenates in 'chars' mode. Accepts keyword names and \\t/\\n escapes; 'newline' puts each piece on its own line."},
            "trim":{"type":"boolean","default":false,"description":"Trim surrounding whitespace from each extracted field ('fields' mode). Default false."},
            "skip_empty_lines":{"type":"boolean","default":false,"description":"Drop blank or whitespace-only input lines instead of emitting an empty output line. Default false."},
            "skip_header":{"type":"boolean","default":false,"description":"Drop the first line of input (a header row) before extracting. Default false."}
          },"required":["text","selectors"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
