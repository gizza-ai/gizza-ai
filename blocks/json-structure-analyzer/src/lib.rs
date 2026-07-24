//! gizza-ai/json-structure-analyzer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_structure_analyzer_core::{analyze, Format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_top_keys")]
    top_keys: u64,
    #[serde(default = "default_top_paths")]
    top_paths: u64,
}
fn default_format() -> String {
    "json".to_string()
}
fn default_top_keys() -> u64 {
    30
}
fn default_top_paths() -> u64 {
    50
}

fn format_from(s: &str) -> Format {
    match s.trim().to_ascii_lowercase().as_str() {
        "text" | "txt" | "plain" => Format::Text,
        _ => Format::Json,
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to analyze (object, array, or any JSON value). Parsed once; structural statistics are computed over every node."),
        )
        .param(
            Param::enumv("format", ["json", "text"])
                .default("json")
                .describe("Output shape. 'json' (default) returns a structured report; 'text' returns a human-readable plain-text summary of the same data."),
        )
        .param(
            Param::integer("top_keys")
                .default(30)
                .min(0.0)
                .describe("Max entries in the key-frequency ranking (keys sorted by how often the name occurs). 0 lists every key. Default 30."),
        )
        .param(
            Param::integer("top_paths")
                .default(50)
                .min(0.0)
                .describe("Max entries in the per-path type distribution (paths sorted by node count; array indices collapse to []). 0 lists every path. Default 50."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<String, String> {
    let opts = Options {
        format: format_from(&a.format),
        top_keys: a.top_keys as usize,
        top_paths: a.top_paths as usize,
    };
    analyze(&a.json, &opts)
}

#[cfg(target_arch = "wasm32")]
struct JsonStructureAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-structure-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze a JSON document's structure: depth, key frequency, per-path types, array stats",
    skill(
        description = "Analyze the structure of a JSON document (however large) without transforming it. Parses the input and reports: max nesting depth; node counts by type (objects, arrays, strings, numbers, booleans, nulls) plus total/unique keys, total values, and empty values (nulls, empty strings, empty containers); a key-frequency ranking (how often each key name occurs, recurring names counted each time); a per-path type distribution (each path with the JSON types seen there, with array indices collapsed to [] so all elements of an array share one path); array-length stats (count, min/max/avg length, total elements); byte size (raw vs minified, plus compression potential); and quality warnings (deep nesting > 5 levels, recurring keys, empty values). Options: format (json default, or text for a human-readable summary), top_keys (cap the key-frequency list, 0 = all, default 30), top_paths (cap the path list, 0 = all, default 50).",
        parameters = schema_json()
    ),
)]
impl JsonStructureAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-structure-analyzer", |a: Args| {
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
                    "json":      { "type": "string", "description": "The JSON document to analyze (object, array, or any JSON value). Parsed once; structural statistics are computed over every node." },
                    "format":    { "type": "string", "enum": ["json", "text"], "default": "json", "description": "Output shape. 'json' (default) returns a structured report; 'text' returns a human-readable plain-text summary of the same data." },
                    "top_keys":  { "type": "integer", "minimum": 0, "default": 30, "description": "Max entries in the key-frequency ranking (keys sorted by how often the name occurs). 0 lists every key. Default 30." },
                    "top_paths": { "type": "integer", "minimum": 0, "default": 50, "description": "Max entries in the per-path type distribution (paths sorted by node count; array indices collapse to []). 0 lists every path. Default 50." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_produces_json_report() {
        let a = Args {
            json: r#"{"a":1,"b":[2,3]}"#.to_string(),
            format: "json".to_string(),
            top_keys: 30,
            top_paths: 50,
        };
        let out = run(a).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["root_type"], "object");
        assert_eq!(v["arrays"]["count"], 1);
    }

    #[test]
    fn run_rejects_invalid() {
        let a = Args {
            json: "not json".to_string(),
            format: "json".to_string(),
            top_keys: 30,
            top_paths: 50,
        };
        assert!(run(a).is_err());
    }
}
