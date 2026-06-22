//! gizza-ai/html-table-extractor — extract <table> data from HTML as CSV or JSON.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_table_extractor_core::{extract, parse_format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    table_index: u64,
    #[serde(default = "default_true")]
    header: bool,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The HTML containing one or more <table> elements."))
        .param(Param::enumv("format", ["csv", "json"]).default("csv").describe("Output format: csv (default) or json."))
        .param(Param::integer("table_index").min(0.0).describe("Which table to extract, 0-based (default 0 = the first table)."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header. For JSON, true emits an array of objects keyed by header; false emits an array of arrays. Default true."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlTableExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-table-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract HTML tables as CSV or JSON",
    skill(
        description = "Extract a <table> from pasted HTML and output it as CSV or JSON. Set format='csv' (default) or 'json', table_index to pick which table (0-based, default 0), and header (default true: for JSON, emits an array of objects keyed by the header row; false emits an array of arrays). Cell whitespace is collapsed and CSV is properly escaped.",
        parameters = schema_json()
    )
)]
impl HtmlTableExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-table-extractor", |a: Args| {
            let fmt = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            extract(&a.html, a.table_index as usize, fmt, a.header).map_err(SkillError::InvalidArgs)
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
                    "html":        { "type": "string", "description": "The HTML containing one or more <table> elements." },
                    "format":      { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format: csv (default) or json." },
                    "table_index": { "type": "integer", "minimum": 0, "description": "Which table to extract, 0-based (default 0 = the first table)." },
                    "header":      { "type": "boolean", "default": true, "description": "Treat the first row as a header. For JSON, true emits an array of objects keyed by header; false emits an array of arrays. Default true." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
