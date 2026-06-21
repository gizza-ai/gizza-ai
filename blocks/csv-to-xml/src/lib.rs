//! gizza-ai/csv-to-xml — convert a CSV table into XML records. Thin wrapper; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_xml_core::to_xml;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_root")]
    root: String,
    #[serde(default = "default_row")]
    row: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_root() -> String {
    "rows".to_string()
}
fn default_row() -> String {
    "row".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(Param::string("root").default("rows").describe("Root element tag wrapping all records (default 'rows')."))
        .param(Param::string("row").default("row").describe("Element tag for each record (default 'row')."))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Use the first row as the field tag names (default true); otherwise 'col1'… are used."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToXml;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-xml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV into XML records",
    skill(
        description = "Convert a CSV table into XML: a `root` element (default 'rows') wraps one `row` element per record, and each field is <tag>value</tag> using the header name as the tag (sanitized to a valid XML name) — or col1… when header=false. Values are XML-escaped. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvToXml {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-xml", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            to_xml(&a.data, &a.root, &a.row, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV text." },
                    "root": { "type": "string", "default": "rows", "description": "Root element tag wrapping all records (default 'rows')." },
                    "row": { "type": "string", "default": "row", "description": "Element tag for each record (default 'row')." },
                    "header": { "type": "boolean", "default": true, "description": "Use the first row as the field tag names (default true); otherwise 'col1'… are used." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
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
