//! gizza-ai/xml-structure-analyzer — analyze XML document shape without transforming it.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xml_structure_analyzer_core::{analyze, Format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    tree_depth: usize,
    #[serde(default = "default_top_tags")]
    top_tags: usize,
    #[serde(default = "default_show_attributes")]
    show_attributes: bool,
}

fn default_format() -> String {
    "text".to_string()
}
fn default_top_tags() -> usize {
    50
}
fn default_show_attributes() -> bool {
    true
}

fn format_from(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "text" | "txt" | "plain" => Ok(Format::Text),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        other => Err(format!(
            "unknown format '{other}' (expected text, json, or csv)"
        )),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xml").required().describe("XML document to analyze. Paste a complete document or fragment with a single root element; namespaces, comments, CDATA, processing instructions, declaration, and DOCTYPE are inspected."))
        .param(Param::enumv("format", ["text", "json", "csv"]).default("text").describe("Output shape. Text renders a human-readable report and tree; JSON returns the complete structured report; CSV returns one row per distinct element tag."))
        .param(Param::integer("tree_depth").default(0).min(0.0).max(20.0).describe("Maximum element-tree levels to render. 0 renders the full collapsed tree; 1 shows only the root level."))
        .param(Param::integer("top_tags").default(50).min(0.0).max(200.0).describe("Maximum tag-frequency rows to include. 0 lists every distinct tag."))
        .param(Param::boolean("show_attributes").default(true).describe("Show attribute names in the tree and include attribute-usage tables. Counts still include attributes when this is false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        format: format_from(&a.format)?,
        tree_depth: a.tree_depth,
        top_tags: a.top_tags,
        show_attributes: a.show_attributes,
    };
    analyze(&a.xml, &opts)
}

#[cfg(target_arch = "wasm32")]
struct XmlStructureAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-structure-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze XML structure: element tree, tag counts, depth, attributes, namespaces.",
    skill(
        description = "Analyze the structure of an XML document without transforming it. Reports root element, element tree, tag counts, max and average depth, depth histogram, attribute usage, namespaces, XML declaration/DOCTYPE, node-type counts, and structural warnings. Options control output format, tree-depth cap, tag table cap, and whether attribute details are shown.",
        parameters = schema_json()
    ),
)]
impl XmlStructureAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-structure-analyzer", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
                    "xml": { "type": "string", "description": "XML document to analyze. Paste a complete document or fragment with a single root element; namespaces, comments, CDATA, processing instructions, declaration, and DOCTYPE are inspected." },
                    "format": { "type": "string", "enum": ["text", "json", "csv"], "default": "text", "description": "Output shape. Text renders a human-readable report and tree; JSON returns the complete structured report; CSV returns one row per distinct element tag." },
                    "tree_depth": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "Maximum element-tree levels to render. 0 renders the full collapsed tree; 1 shows only the root level." },
                    "top_tags": { "type": "integer", "minimum": 0, "maximum": 200, "default": 50, "description": "Maximum tag-frequency rows to include. 0 lists every distinct tag." },
                    "show_attributes": { "type": "boolean", "default": true, "description": "Show attribute names in the tree and include attribute-usage tables. Counts still include attributes when this is false." }
                },
                "required": ["xml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_produces_text_report() {
        let out = run(Args {
            xml: "<root><item id=\"a\">x</item></root>".to_string(),
            format: "text".to_string(),
            tree_depth: 0,
            top_tags: 50,
            show_attributes: true,
        })
        .unwrap();
        assert!(out.contains("Root element:  root"));
        assert!(out.contains("item (1)  [id]"));
    }
}
