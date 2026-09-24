//! gizza-ai/xsd-from-xml — infer an XSD schema from one XML sample.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default = "default_design")]
    design: String,
    #[serde(default = "default_type_inference")]
    type_inference: String,
    #[serde(default = "default_occurrence")]
    occurrence: String,
    #[serde(default)]
    enumerations: f64,
    #[serde(default)]
    target_namespace: String,
    #[serde(default = "default_indent")]
    indent: f64,
    #[serde(default = "default_declaration")]
    declaration: bool,
}

fn default_design() -> String {
    "venetian-blind".into()
}
fn default_type_inference() -> String {
    "smart".into()
}
fn default_occurrence() -> String {
    "restricted".into()
}
fn default_indent() -> f64 {
    2.0
}
fn default_declaration() -> bool {
    true
}

const XML_DESC: &str = "A representative XML instance document to infer from. Paste one complete document with a single root element. Repeated siblings are used to infer maxOccurs, missing siblings across repeated parents become minOccurs=0, attributes observed on every instance become required in restricted mode, and values are scanned for booleans, integers, decimals, dates, dateTimes, times, and URIs. The sample is capped at 1000000 bytes; use a concise excerpt that includes every distinct element and attribute shape you need.";
const DESIGN_DESC: &str = "Schema layout. venetian-blind (default) emits one global root element plus named complex types, which handles recursive structures and keeps the schema readable. salami-slice emits every element globally and references children. russian-doll nests everything inline under the root; it is compact for tiny non-recursive samples but rejects recursive shapes because inline recursive types cannot be expressed safely.";
const TYPE_DESC: &str = "Value typing mode. smart (default) infers XML Schema builtins such as xs:boolean, xs:int, xs:long, xs:integer, xs:decimal, xs:double, xs:date, xs:dateTime, xs:time, xs:anyURI, and xs:string. string declares every simple value as xs:string when you want a deliberately loose draft.";
const OCCURRENCE_DESC: &str = "How tight to make minOccurs/maxOccurs and attribute use. restricted follows the sample: always-present children are required, missing children are optional, repeated children are unbounded, and attributes present on every element are required. relaxed makes child elements optional and repeatable and never marks attributes required, which is safer when one sample may not show every optional field.";
const ENUM_DESC: &str = "Maximum distinct values to turn into xs:enumeration facets. Default 0 disables enumeration inference because one XML sample rarely proves the whole domain. Set 2-20 for small known code lists such as status values; values above the cap stay as the inferred base type.";
const NS_DESC: &str = "Optional targetNamespace override for the generated schema. Leave blank to use the sample root's default namespace when it has one, or no targetNamespace for namespace-free XML. When a target namespace is present, generated references use the tns prefix and elementFormDefault=qualified.";
const INDENT_DESC: &str = "Spaces per XSD indentation level, 0-8. Default 2. Set 0 for compact output or 4 when matching editors that prefer wider indentation.";
const DECL_DESC: &str = "Whether to include the XML declaration `<?xml version=\"1.0\" encoding=\"UTF-8\"?>` before the xs:schema element. Default true.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xml").required().describe(XML_DESC))
        .param(
            Param::enumv("design", ["venetian-blind", "salami-slice", "russian-doll"])
                .default("venetian-blind")
                .describe(DESIGN_DESC),
        )
        .param(
            Param::enumv("type_inference", ["smart", "string"])
                .default("smart")
                .describe(TYPE_DESC),
        )
        .param(
            Param::enumv("occurrence", ["restricted", "relaxed"])
                .default("restricted")
                .describe(OCCURRENCE_DESC),
        )
        .param(
            Param::integer("enumerations")
                .min(0.0)
                .max(1000.0)
                .default(0)
                .describe(ENUM_DESC),
        )
        .param(
            Param::string("target_namespace")
                .default("")
                .describe(NS_DESC),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe(INDENT_DESC),
        )
        .param(
            Param::boolean("declaration")
                .default(true)
                .describe(DECL_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xsd-from-xml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer an XSD schema from a sample XML document with layout, occurrence, namespace, and type controls",
    skill(
        description = "Generate a draft W3C XML Schema (XSD) from one representative XML instance document. The tool parses the XML locally, merges repeated elements, infers child and attribute cardinality, detects namespaces, marks mixed content and xsi:nil, infers simple XML Schema types, and emits the schema in venetian-blind, salami-slice, or russian-doll layout. Options control smart-vs-string type inference, restricted-vs-relaxed occurrence bounds, optional enumeration facets, target namespace override, indentation, and whether to include the XML declaration. A single sample cannot prove every domain rule, so the output is a draft schema to review before using in production.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xsd-from-xml", |a: Args| {
            gizza_ai_xsd_from_xml_core::run(
                &a.xml,
                &a.design,
                &a.type_inference,
                &a.occurrence,
                a.enumerations,
                &a.target_namespace,
                a.indent,
                a.declaration,
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
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8);
        for (name, spec) in props {
            assert!(
                spec["description"].as_str().unwrap_or("").len() > 20,
                "{name}"
            );
        }
        assert_eq!(
            schema["required"].as_array().unwrap(),
            &vec![serde_json::json!("xml")]
        );
    }

    #[test]
    fn schema_matches_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["xml"],
            "properties": {
                "xml": { "type": "string", "description": XML_DESC },
                "design": { "type": "string", "enum": ["venetian-blind", "salami-slice", "russian-doll"], "default": "venetian-blind", "description": DESIGN_DESC },
                "type_inference": { "type": "string", "enum": ["smart", "string"], "default": "smart", "description": TYPE_DESC },
                "occurrence": { "type": "string", "enum": ["restricted", "relaxed"], "default": "restricted", "description": OCCURRENCE_DESC },
                "enumerations": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 0, "description": ENUM_DESC },
                "target_namespace": { "type": "string", "default": "", "description": NS_DESC },
                "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": INDENT_DESC },
                "declaration": { "type": "boolean", "default": true, "description": DECL_DESC }
            }
        });
        assert_eq!(actual, authored);
    }

    #[test]
    fn defaults_run_through_core() {
        let a: Args =
            serde_json::from_str(r#"{"xml":"<order id=\"7\"><item>pen</item></order>"}"#).unwrap();
        let out = gizza_ai_xsd_from_xml_core::run(
            &a.xml,
            &a.design,
            &a.type_inference,
            &a.occurrence,
            a.enumerations,
            &a.target_namespace,
            a.indent,
            a.declaration,
        )
        .unwrap();
        assert!(out.contains("<xs:element name=\"order\""), "{out}");
        assert!(
            out.contains("<xs:attribute name=\"id\" type=\"xs:int\" use=\"required\"/>"),
            "{out}"
        );
    }
}
