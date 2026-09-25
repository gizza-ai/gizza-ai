//! gizza-ai/pdf-structure-inspector — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and the pure core parser.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pdf_structure_inspector_core::{
    Options, DEFAULT_MAX_OBJECTS, MAX_MAX_OBJECTS, MIN_MAX_OBJECTS,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "d_section")]
    section: String,
    #[serde(default)]
    object_id: String,
    #[serde(default)]
    filter_key: String,
    #[serde(default = "d_max_objects")]
    max_objects: u32,
    #[serde(default = "d_format")]
    format: String,
}

fn d_section() -> String {
    "all".into()
}
fn d_max_objects() -> u32 {
    DEFAULT_MAX_OBJECTS
}
fn d_format() -> String {
    "text".into()
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            section: a.section,
            object_id: a.object_id,
            filter_key: a.filter_key,
            max_objects: a.max_objects,
            format: a.format,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Required. The PDF file bytes as base64, hex, or a data:application/pdf;base64,… URL. The tool parses structure only; it does not render pages or dump stream bodies."))
        .param(Param::enumv("section", ["all", "summary", "trailer", "objects", "streams"]).default("all").describe("Which part of the structure report to return. all includes summary, trailer and object rows; streams lists only stream objects."))
        .param(Param::string("object_id").describe("Optional object selector. Use a bare object number like 12 to match any generation, or an exact id like 12 0."))
        .param(Param::string("filter_key").describe("Optional dictionary key or /Type value filter. Examples: Type, Font, Page, Resources, /XObject. Matching ignores case and a leading slash."))
        .param(Param::integer("max_objects").min(MIN_MAX_OBJECTS as f64).max(MAX_MAX_OBJECTS as f64).default(DEFAULT_MAX_OBJECTS as i64).describe("Maximum number of matching objects to list in the report. The summary counts still describe the whole file. Range 1-5000; default 100."))
        .param(Param::enumv("format", ["text", "json"]).default("text").describe("Output format. text is a readable report; json returns the same structural facts as machine-readable JSON."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-structure-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a PDF's object tree, trailer and stream dictionaries without rendering pages.",
    skill(
        description = "Parse a PDF's file structure from pasted base64, hex, or a data:application/pdf;base64 URL. Reports the PDF version, page count, object and stream counts, trailer entries, cross-reference style, encryption and linearization flags, per-object kind/type/subtype/dictionary keys, and stream /Filter and /Length data. Parameters: input (required), section=all|summary|trailer|objects|streams, object_id such as 12 or 12 0, filter_key such as Type or Font, max_objects 1-5000 default 100, and format=text|json. This is structural inspection only: it does not render pages, decrypt files, execute embedded JavaScript, or dump arbitrary stream bodies.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pdf-structure-inspector", |a: Args| {
            let input = a.input.clone();
            gizza_ai_pdf_structure_inspector_core::run(&input, &Options::from(a))
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
    fn args_defaults_match_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"JVBERi0="}"#).unwrap();
        assert_eq!(a.section, "all");
        assert_eq!(a.object_id, "");
        assert_eq!(a.filter_key, "");
        assert_eq!(a.max_objects, DEFAULT_MAX_OBJECTS);
        assert_eq!(a.format, "text");
    }

    #[test]
    fn args_flow_into_core_options() {
        let a: Args = serde_json::from_str(r#"{"input":"x","section":"streams","object_id":"5 0","filter_key":"Font","max_objects":12,"format":"json"}"#).unwrap();
        let o = Options::from(a);
        assert_eq!(o.section, "streams");
        assert_eq!(o.object_id, "5 0");
        assert_eq!(o.filter_key, "Font");
        assert_eq!(o.max_objects, 12);
        assert_eq!(o.format, "json");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "input":{"type":"string","description":"Required. The PDF file bytes as base64, hex, or a data:application/pdf;base64,… URL. The tool parses structure only; it does not render pages or dump stream bodies."},
                "section":{"type":"string","enum":["all","summary","trailer","objects","streams"],"default":"all","description":"Which part of the structure report to return. all includes summary, trailer and object rows; streams lists only stream objects."},
                "object_id":{"type":"string","description":"Optional object selector. Use a bare object number like 12 to match any generation, or an exact id like 12 0."},
                "filter_key":{"type":"string","description":"Optional dictionary key or /Type value filter. Examples: Type, Font, Page, Resources, /XObject. Matching ignores case and a leading slash."},
                "max_objects":{"type":"integer","minimum":1,"maximum":5000,"default":100,"description":"Maximum number of matching objects to list in the report. The summary counts still describe the whole file. Range 1-5000; default 100."},
                "format":{"type":"string","enum":["text","json"],"default":"text","description":"Output format. text is a readable report; json returns the same structural facts as machine-readable JSON."}
            },
            "required":["input"],
            "additionalProperties":false
        }"#).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, authored);
    }
}
