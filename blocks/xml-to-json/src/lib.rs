//! gizza-ai/xml-to-json — convert XML into an equivalent JSON structure,
//! preserving attributes and nesting. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xml_to_json_core::{to_json, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default = "default_attr_prefix")]
    attribute_prefix: String,
    #[serde(default = "default_text_key")]
    text_key: String,
    #[serde(default = "default_true")]
    attributes: bool,
    #[serde(default)]
    coerce_types: bool,
}
fn default_true() -> bool {
    true
}
fn default_attr_prefix() -> String {
    "@".to_string()
}
fn default_text_key() -> String {
    "#text".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xml").required().describe("The XML document to convert to JSON."))
        .param(
            Param::string("attribute_prefix")
                .default("@")
                .describe("Prefix added to JSON keys that come from XML attributes (e.g. '@id'). Default '@'."),
        )
        .param(
            Param::string("text_key")
                .default("#text")
                .describe("Key under which an element's text is stored when it also has attributes or child elements. Default '#text'."),
        )
        .param(
            Param::boolean("attributes")
                .default(true)
                .describe("Include XML attributes in the JSON output. When false, attributes are dropped. Default true."),
        )
        .param(
            Param::boolean("coerce_types")
                .default(false)
                .describe("Coerce text that looks like a number, boolean, or null into the matching JSON scalar instead of a string. Default false (everything stays a string)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XmlToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert XML into an equivalent JSON structure",
    skill(
        description = "Convert an XML document into an equivalent JSON structure, preserving attributes and nesting. Each element becomes a JSON object keyed by its tag; the document's single root element becomes the sole top-level key. XML attributes become object members prefixed by `attribute_prefix` (default '@', so id=\"1\" becomes \"@id\":\"1\"); set attributes=false to drop them. Repeated sibling tags collapse into a JSON array in document order. An element with only text becomes a plain string; when it also has attributes or children its text is stored under `text_key` (default '#text'). XML entities and CDATA are decoded, comments and processing-instructions are ignored, and namespace prefixes are stripped to local names. With coerce_types=true, text that looks like an integer, float, boolean, or null is emitted as the matching JSON scalar (leading-zero strings like '007' stay strings). Output is pretty-printed JSON. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl XmlToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-to-json", |a: Args| {
            let opt = Options {
                attr_prefix: if a.attribute_prefix.is_empty() {
                    "@".to_string()
                } else {
                    a.attribute_prefix
                },
                text_key: if a.text_key.is_empty() {
                    "#text".to_string()
                } else {
                    a.text_key
                },
                include_attributes: a.attributes,
                coerce: a.coerce_types,
            };
            to_json(&a.xml, &opt).map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "xml": { "type": "string", "description": "The XML document to convert to JSON." },
                    "attribute_prefix": { "type": "string", "default": "@", "description": "Prefix added to JSON keys that come from XML attributes (e.g. '@id'). Default '@'." },
                    "text_key": { "type": "string", "default": "#text", "description": "Key under which an element's text is stored when it also has attributes or child elements. Default '#text'." },
                    "attributes": { "type": "boolean", "default": true, "description": "Include XML attributes in the JSON output. When false, attributes are dropped. Default true." },
                    "coerce_types": { "type": "boolean", "default": false, "description": "Coerce text that looks like a number, boolean, or null into the matching JSON scalar instead of a string. Default false (everything stays a string)." }
                },
                "required": ["xml"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
