//! gizza-ai/json-to-xml — render a JSON value as well-formed XML. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_xml_core::{to_xml, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_root")]
    root_element: String,
    #[serde(default = "default_item")]
    array_item_element: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default)]
    xml_declaration: bool,
    #[serde(default = "default_attr_prefix")]
    attribute_prefix: String,
    #[serde(default = "default_text_key")]
    text_key: String,
}
fn default_root() -> String {
    "root".to_string()
}
fn default_item() -> String {
    "item".to_string()
}
fn default_format() -> String {
    "pretty".to_string()
}
fn default_indent() -> i64 {
    2
}
fn default_attr_prefix() -> String {
    "@".to_string()
}
fn default_text_key() -> String {
    "#text".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe("The JSON text to convert to XML."))
        .param(
            Param::string("root_element")
                .default("root")
                .describe("Name of the single root element that wraps the output (sanitized to a valid XML name). Default 'root'."),
        )
        .param(
            Param::string("array_item_element")
                .default("item")
                .describe("Element name wrapping each item of a JSON array — an array under 'books' becomes <books><item>…</item></books>. Default 'item'."),
        )
        .param(
            Param::enumv("format", ["pretty", "compact"])
                .default("pretty")
                .describe("Output style: 'pretty' (indented, multi-line) or 'compact' (single line, no whitespace). Default 'pretty'."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Number of spaces per indent level when format is 'pretty' (0-8). Ignored when compact. Default 2."),
        )
        .param(
            Param::boolean("xml_declaration")
                .default(false)
                .describe("Prepend an XML declaration <?xml version=\"1.0\" encoding=\"UTF-8\"?>. Default false."),
        )
        .param(
            Param::string("attribute_prefix")
                .default("@")
                .describe("Object keys starting with this prefix become XML attributes on their parent element instead of child elements (e.g. '@id' → id=\"…\"). Set empty to disable. Default '@'."),
        )
        .param(
            Param::string("text_key")
                .default("#text")
                .describe("Object key whose scalar value becomes the parent element's text content when the element also has attributes or children. Default '#text'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToXml;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-xml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a JSON object as well-formed XML",
    skill(
        description = "Render a JSON document as well-formed XML. The whole value is wrapped in one `root_element` (default 'root'). Each object member becomes a child element <key>value</key> (keys are sanitized to valid XML names). A member whose key starts with `attribute_prefix` (default '@') and has a scalar value becomes an XML attribute on its parent element instead of a child (so \"@id\":\"1\" becomes id=\"1\"; set the prefix empty to disable). A member named `text_key` (default '#text') with a scalar value becomes the element's text content, enabling mixed attribute+text content. Each item of a JSON array is emitted as an `array_item_element` (default 'item') child. Strings, numbers, and booleans become element text; null and empty objects/arrays become empty self-closing elements. format='pretty' indents by `indent` spaces per level (0-8); 'compact' emits a single line. Set xml_declaration=true to prepend the XML declaration. Text and attribute values are XML-escaped. This is the inverse of xml-to-json and round-trips with it when the attribute prefix and text key match. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl JsonToXml {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-xml", |a: Args| {
            let opt = Options {
                root_element: if a.root_element.is_empty() {
                    "root".to_string()
                } else {
                    a.root_element
                },
                array_item_element: if a.array_item_element.is_empty() {
                    "item".to_string()
                } else {
                    a.array_item_element
                },
                pretty: a.format != "compact",
                indent: a.indent.clamp(0, 8) as usize,
                xml_declaration: a.xml_declaration,
                attribute_prefix: a.attribute_prefix,
                text_key: a.text_key,
            };
            to_xml(&a.json, &opt).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// literal exactly (keeps the LLM-facing schema stable and reviewed).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON text to convert to XML." },
                    "root_element": { "type": "string", "default": "root", "description": "Name of the single root element that wraps the output (sanitized to a valid XML name). Default 'root'." },
                    "array_item_element": { "type": "string", "default": "item", "description": "Element name wrapping each item of a JSON array — an array under 'books' becomes <books><item>…</item></books>. Default 'item'." },
                    "format": { "type": "string", "enum": ["pretty", "compact"], "default": "pretty", "description": "Output style: 'pretty' (indented, multi-line) or 'compact' (single line, no whitespace). Default 'pretty'." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Number of spaces per indent level when format is 'pretty' (0-8). Ignored when compact. Default 2." },
                    "xml_declaration": { "type": "boolean", "default": false, "description": "Prepend an XML declaration <?xml version=\"1.0\" encoding=\"UTF-8\"?>. Default false." },
                    "attribute_prefix": { "type": "string", "default": "@", "description": "Object keys starting with this prefix become XML attributes on their parent element instead of child elements (e.g. '@id' → id=\"…\"). Set empty to disable. Default '@'." },
                    "text_key": { "type": "string", "default": "#text", "description": "Object key whose scalar value becomes the parent element's text content when the element also has attributes or children. Default '#text'." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
