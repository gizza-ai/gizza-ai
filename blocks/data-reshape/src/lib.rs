//! gizza-ai/data-reshape — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends incl.
//! chat SW. Parses JSON/YAML/CSV/XML and reshapes it with a JSONata query/template.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_reshape_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    query: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_json")]
    output_format: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_json() -> String {
    "json".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The source document to reshape (JSON, YAML, CSV, or XML)."),
        )
        .param(Param::string("query").required().describe(
            "The JSONata expression that queries and/or templates the data, e.g. '$sum(items.price)', 'items[price > 10].name', or '{ \"total\": $sum(lines.amount) }'. CSV rows arrive as an array of objects; XML/YAML map to nested objects.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "json", "yaml", "csv", "xml"])
                .default("auto")
                .describe("Format of the input document. 'auto' sniffs it (< → XML, {/[ → JSON, a comma header → CSV, else YAML)."),
        )
        .param(
            Param::enumv("output_format", ["json", "yaml"])
                .default("json")
                .describe("Format of the reshaped result. Default json."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the JSON output (ignored for YAML). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DataReshape;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-reshape",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reshape JSON/YAML/CSV/XML with a JSONata query",
    skill(
        description = "Parse a document in JSON, YAML, CSV, or XML and reshape it with a JSONata expression into a new structured result (JSON or YAML). JSONata is one language for both querying (navigate paths, filter with predicates, aggregate with $sum/$count/$max) and templating (construct new objects/arrays like '{ \"total\": $sum(lines.amount) }'). CSV parses to an array of row objects (numeric/boolean cells coerced); XML/YAML map to nested objects. Set input_format=auto to sniff the format. A query that matches nothing returns null. Runs locally, pure-Rust engine.",
        parameters = schema_json()
    ),
)]
impl DataReshape {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-reshape", |a: Args| {
            run(&a.data, &a.query, &a.input_format, &a.output_format, a.pretty)
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":          { "type": "string", "description": "The source document to reshape (JSON, YAML, CSV, or XML)." },
                    "query":         { "type": "string", "description": "The JSONata expression that queries and/or templates the data, e.g. '$sum(items.price)', 'items[price > 10].name', or '{ \"total\": $sum(lines.amount) }'. CSV rows arrive as an array of objects; XML/YAML map to nested objects." },
                    "input_format":  { "type": "string", "enum": ["auto", "json", "yaml", "csv", "xml"], "default": "auto", "description": "Format of the input document. 'auto' sniffs it (< → XML, {/[ → JSON, a comma header → CSV, else YAML)." },
                    "output_format": { "type": "string", "enum": ["json", "yaml"], "default": "json", "description": "Format of the reshaped result. Default json." },
                    "pretty":        { "type": "boolean", "default": true, "description": "Indent the JSON output (ignored for YAML). Default true." }
                },
                "required": ["data", "query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
