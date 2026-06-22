//! gizza-ai/xml-to-csv — flatten repeated XML elements into a CSV table. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xml_to_csv_core::to_csv;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default)]
    record: String,
    #[serde(default = "default_true")]
    attributes: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xml").required().describe("The XML text to flatten."))
        .param(
            Param::string("record")
                .describe("The repeated element tag to treat as a row (e.g. 'book'). Leave empty to auto-detect the most frequent child of the root."),
        )
        .param(
            Param::boolean("attributes")
                .default(true)
                .describe("Include attributes as columns: '@attr' on the record element, 'child@attr' on a child. Default true."),
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
struct XmlToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten repeated XML elements into a CSV table",
    skill(
        description = "Convert XML into a CSV table: pick the repeated `record` element (e.g. <book> inside <catalog>) — auto-detected as the most frequent child of the root when omitted — and turn each one into a CSV row. Columns are inferred, in first-seen order, from the record's child element tags; nested elements flatten to dot-notation columns (<address><city> → 'address.city'). With attributes=true (default) each attribute also becomes a column ('@attr' on the record, 'path@attr' on a nested element). Repeated tags get .2/.3 suffixes, missing fields become empty cells, XML entities and CDATA are decoded, and values are CSV-quoted as needed. delimiter is a single char or comma/tab/semicolon/pipe. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl XmlToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-to-csv", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            to_csv(&a.xml, &a.record, a.attributes, &delim).map_err(SkillError::InvalidArgs)
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
                    "xml": { "type": "string", "description": "The XML text to flatten." },
                    "record": { "type": "string", "description": "The repeated element tag to treat as a row (e.g. 'book'). Leave empty to auto-detect the most frequent child of the root." },
                    "attributes": { "type": "boolean", "default": true, "description": "Include attributes as columns: '@attr' on the record element, 'child@attr' on a child. Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["xml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
