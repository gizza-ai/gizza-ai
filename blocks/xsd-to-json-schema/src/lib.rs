//! gizza-ai/xsd-to-json-schema — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xsd_to_json_schema_core::{convert, draft_from_str, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xsd: String,
    #[serde(default)]
    root_element: String,
    #[serde(default = "default_draft")]
    draft: String,
    #[serde(default = "default_attribute_prefix")]
    attribute_prefix: String,
    #[serde(default = "default_text_property")]
    text_property: String,
    #[serde(default = "default_true")]
    required_from_occurs: bool,
    #[serde(default)]
    additional_properties: bool,
    #[serde(default = "default_true")]
    annotations: bool,
}
fn default_draft() -> String {
    "2020-12".to_string()
}
fn default_attribute_prefix() -> String {
    "@".to_string()
}
fn default_text_property() -> String {
    "#text".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("xsd")
                .required()
                .describe("The XML Schema document to convert, as text — a whole .xsd file starting with <xs:schema> (any prefix bound to http://www.w3.org/2001/XMLSchema works, including a default namespace). Supported: xs:element, xs:complexType, xs:simpleType, xs:attribute, xs:group, xs:attributeGroup, xs:sequence/xs:all/xs:choice, minOccurs/maxOccurs, complexContent and simpleContent extension/restriction, facets (enumeration, pattern, length, min/maxInclusive, min/maxExclusive, fractionDigits), xs:list, xs:union, nillable, default and fixed. Maximum 1000000 bytes. Example: '<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"><xs:element name=\"id\" type=\"xs:string\"/></xs:schema>'."),
        )
        .param(
            Param::string("root_element")
                .default("")
                .describe("Name of the global xs:element (or named xs:complexType / xs:simpleType) to use as the schema root, e.g. 'order'. Empty (default) uses the FIRST global element, or the first named complexType when the schema declares no global element. Other named types are emitted under $defs/definitions only when the root actually references them."),
        )
        .param(
            Param::enumv("draft", ["2020-12", "draft-07"])
                .default("2020-12")
                .describe("JSON Schema dialect to emit. '2020-12' (default) uses $defs and allows keywords next to $ref; 'draft-07' uses definitions and wraps a $ref in allOf when it carries a description. Also sets the $schema URL."),
        )
        .param(
            Param::string("attribute_prefix")
                .default("@")
                .describe("Prefix added to property names that come from XML attributes, so they cannot collide with child elements. Default '@' (xs:attribute name=\"currency\" becomes the property '@currency'). Set to an empty string for unprefixed names."),
        )
        .param(
            Param::string("text_property")
                .default("#text")
                .describe("Property name that holds element text for types with simpleContent (text plus attributes) or mixed=\"true\". Default '#text'. Set to an empty string to drop the text property entirely and keep only the attributes."),
        )
        .param(
            Param::boolean("required_from_occurs")
                .default(true)
                .describe("Derive 'required' from the schema: elements with minOccurs >= 1 and attributes with use=\"required\" are listed, and an xs:choice becomes a 'oneOf' over its members' required sets. Default true; false omits 'required' and the choice constraint entirely."),
        )
        .param(
            Param::boolean("additional_properties")
                .default(false)
                .describe("Allow properties beyond those declared. false (default) emits 'additionalProperties: false' for strict validation; true omits it. Types containing xs:any or xs:anyAttribute are always left open regardless of this setting."),
        )
        .param(
            Param::boolean("annotations")
                .default(true)
                .describe("Turn xs:annotation/xs:documentation text into JSON Schema 'description' strings on the matching element, attribute or type. Default true; false ignores all annotations."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XsdToJsonSchema;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xsd-to-json-schema",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an XML Schema (XSD) document into an equivalent JSON Schema",
    skill(
        description = "Translate a pasted XML Schema (.xsd) document into an equivalent JSON Schema, in Draft 2020-12 or Draft-07. xs:complexType becomes an object with properties; xs:sequence and xs:all contribute properties in document order; xs:choice becomes a real oneOf over its members' required sets; xs:attribute becomes a property named with a configurable prefix (default '@') and use=\"required\" makes it required; minOccurs/maxOccurs drive the required list and array wrapping with minItems/maxItems; xs:simpleType restrictions map onto enum, pattern, minLength/maxLength, minimum/maximum, exclusiveMinimum/exclusiveMaximum and multipleOf; xs:list becomes an array and xs:union becomes anyOf; complexContent and simpleContent extension merge the base type's members, with simpleContent text stored under a configurable text property (default '#text'); nillable adds null; default and fixed become default and const; and named global types become $ref entries in $defs/definitions, pruned to what the chosen root reaches, so recursive types work. Builtin xs:* datatypes map to JSON types with the ranges and formats XSD defines (xs:int bounds, xs:date as format date, xs:base64Binary as base64 contentEncoding, and so on). Options: root_element, draft, attribute_prefix, text_property, required_from_occurs, additional_properties, annotations. Cross-document features are not resolved — xs:import, xs:include, xs:redefine and substitution groups need the other documents, and XSD 1.1 xs:assert needs XPath; an unresolvable reference returns a named error instead of a guess. Input is capped at 1000000 bytes. Returns the pretty-printed JSON Schema.",
        parameters = schema_json()
    ),
)]
impl XsdToJsonSchema {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xsd-to-json-schema", |a: Args| {
            let opts = Options {
                draft: draft_from_str(&a.draft),
                root_element: a.root_element,
                attribute_prefix: a.attribute_prefix,
                text_property: a.text_property,
                required_from_occurs: a.required_from_occurs,
                additional_properties: a.additional_properties,
                annotations: a.annotations,
            };
            convert(&a.xsd, &opts).map_err(SkillError::InvalidArgs)
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
                    "xsd":                   { "type": "string", "description": "The XML Schema document to convert, as text — a whole .xsd file starting with <xs:schema> (any prefix bound to http://www.w3.org/2001/XMLSchema works, including a default namespace). Supported: xs:element, xs:complexType, xs:simpleType, xs:attribute, xs:group, xs:attributeGroup, xs:sequence/xs:all/xs:choice, minOccurs/maxOccurs, complexContent and simpleContent extension/restriction, facets (enumeration, pattern, length, min/maxInclusive, min/maxExclusive, fractionDigits), xs:list, xs:union, nillable, default and fixed. Maximum 1000000 bytes. Example: '<xs:schema xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"><xs:element name=\"id\" type=\"xs:string\"/></xs:schema>'." },
                    "root_element":          { "type": "string", "default": "", "description": "Name of the global xs:element (or named xs:complexType / xs:simpleType) to use as the schema root, e.g. 'order'. Empty (default) uses the FIRST global element, or the first named complexType when the schema declares no global element. Other named types are emitted under $defs/definitions only when the root actually references them." },
                    "draft":                 { "type": "string", "enum": ["2020-12", "draft-07"], "default": "2020-12", "description": "JSON Schema dialect to emit. '2020-12' (default) uses $defs and allows keywords next to $ref; 'draft-07' uses definitions and wraps a $ref in allOf when it carries a description. Also sets the $schema URL." },
                    "attribute_prefix":      { "type": "string", "default": "@", "description": "Prefix added to property names that come from XML attributes, so they cannot collide with child elements. Default '@' (xs:attribute name=\"currency\" becomes the property '@currency'). Set to an empty string for unprefixed names." },
                    "text_property":         { "type": "string", "default": "#text", "description": "Property name that holds element text for types with simpleContent (text plus attributes) or mixed=\"true\". Default '#text'. Set to an empty string to drop the text property entirely and keep only the attributes." },
                    "required_from_occurs":  { "type": "boolean", "default": true, "description": "Derive 'required' from the schema: elements with minOccurs >= 1 and attributes with use=\"required\" are listed, and an xs:choice becomes a 'oneOf' over its members' required sets. Default true; false omits 'required' and the choice constraint entirely." },
                    "additional_properties": { "type": "boolean", "default": false, "description": "Allow properties beyond those declared. false (default) emits 'additionalProperties: false' for strict validation; true omits it. Types containing xs:any or xs:anyAttribute are always left open regardless of this setting." },
                    "annotations":           { "type": "boolean", "default": true, "description": "Turn xs:annotation/xs:documentation text into JSON Schema 'description' strings on the matching element, attribute or type. Default true; false ignores all annotations." }
                },
                "required": ["xsd"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
