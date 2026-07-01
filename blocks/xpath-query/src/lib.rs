//! gizza-ai/xpath-query — evaluate an XPath 1.0 expression against an XML/XHTML
//! document. Thin wrapper around the core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure (sxd-xpath) → all backends
//! incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xpath_query_core::{query_xpath, Output};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expression: String,
    xml: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_output() -> String {
    "value".into()
}

#[derive(Serialize)]
struct Resp {
    /// One serialized result per matched node (string value or outer XML, per
    /// `output`). For a scalar XPath result (number/string/boolean) this is a single
    /// element.
    outputs: Vec<String>,
    count: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("expression").required().describe("The XPath 1.0 expression, e.g. '//book/title', '//a/@href', or 'count(//item)'."))
        .param(Param::string("xml").required().describe("The XML or XHTML document to query."))
        .param(Param::enumv("output", ["value", "xml"]).default("value").describe("What to return for each matched node: 'value' = its string value (text content), 'xml' = its serialized outer XML. Default 'value'."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XpathQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xpath-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Evaluate an XPath 1.0 expression against XML",
    skill(
        description = "Evaluate an XPath 1.0 expression against an XML or XHTML document and return the matching nodes or values, using a pure-Rust engine (sxd-xpath). Supports location paths (//book/title), attributes (//a/@href), predicates and filters (//book[price < 10]), axes (ancestor/following-sibling/…), and the XPath function library (count(), name(), text(), contains(), substring(), …). A node-set query returns one result per matched node (in document order) — set output='value' for each node's text content or output='xml' for its serialized outer XML. A scalar expression (count(//x), name(/*), //a > 1) returns a single value. Example: '//book[@category=\"fiction\"]/title'.",
        parameters = schema_json()
    )
)]
impl XpathQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xpath-query", |a: Args| {
            let output = Output::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            let outputs = query_xpath(&a.expression, &a.xml, output).map_err(SkillError::InvalidArgs)?;
            let count = outputs.len();
            Ok(Resp { outputs, count })
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
                    "expression": { "type": "string", "description": "The XPath 1.0 expression, e.g. '//book/title', '//a/@href', or 'count(//item)'." },
                    "xml":        { "type": "string", "description": "The XML or XHTML document to query." },
                    "output":     { "type": "string", "enum": ["value", "xml"], "default": "value", "description": "What to return for each matched node: 'value' = its string value (text content), 'xml' = its serialized outer XML. Default 'value'." }
                },
                "required": ["expression", "xml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
