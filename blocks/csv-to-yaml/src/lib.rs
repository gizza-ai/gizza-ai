//! gizza-ai/csv-to-yaml — convert a CSV table into a YAML list of objects. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_yaml_core::to_yaml;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    infer_types: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Use the first row as the object keys (default true); otherwise 'col1'… are used."),
        )
        .param(
            Param::boolean("infer_types")
                .default(true)
                .describe("Infer numbers, booleans and null from cell text (default true); false keeps everything as strings."),
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
struct CsvToYaml;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-yaml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV into a YAML list of objects",
    skill(
        description = "Convert a CSV table into a YAML list (sequence) of objects keyed by the header row (or col1… when header=false). Column order is preserved. infer_types=true (default) turns cell text into numbers/booleans/null (leading-zero and signed strings stay text); false keeps strings. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvToYaml {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-yaml", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            to_yaml(&a.data, a.header, a.infer_types, &delim).map_err(SkillError::InvalidArgs)
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
                    "header": { "type": "boolean", "default": true, "description": "Use the first row as the object keys (default true); otherwise 'col1'… are used." },
                    "infer_types": { "type": "boolean", "default": true, "description": "Infer numbers, booleans and null from cell text (default true); false keeps everything as strings." },
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
