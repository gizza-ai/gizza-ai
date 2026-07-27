//! gizza-ai/python-dict-to-json — convert a Python dict/list literal into valid JSON.
//!
//! Thin chat-skill wrapper around `gizza-ai-python-dict-to-json-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across chat
//! + CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    indent: String,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default)]
    ensure_ascii: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Python dict or list literal to convert, e.g. as printed by print(obj) or repr(obj): {'name': 'Ann', 'active': True, 'tags': None}. Accepts single quotes, True/False/None, tuples and sets, trailing commas, # comments, 0x/0o/0b ints and 1_000 separators."),
        )
        .param(
            Param::enumv("indent", ["2", "4", "tab", "minify"])
                .default("2")
                .describe("Output formatting: '2' or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line. Default 2."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Sort every object's keys alphabetically (like Python json.dumps(sort_keys=True)). Default false keeps the original key order."),
        )
        .param(
            Param::boolean("ensure_ascii")
                .default(false)
                .describe("Escape every non-ASCII character as \\uXXXX (Python json.dumps default). Default false emits readable UTF-8 verbatim."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PythonDictToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/python-dict-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Python dict/list literal into valid JSON.",
    skill(
        description = "Convert a Python dict or list literal (as printed by print(obj) or repr(obj)) into valid JSON. Handles the Python conveniences plain JSON rejects: single-quoted strings, True/False/None -> true/false/null, tuples and sets -> arrays, trailing commas, # comments, 0x/0o/0b integer literals and 1_000 digit separators, and non-string dict keys (stringified). Use indent='2' (default), '4', 'tab', or 'minify' for output formatting; sort_keys=true to alphabetize object keys; ensure_ascii=true to escape non-ASCII as \\uXXXX.",
        parameters = schema_json()
    ),
)]
impl PythonDictToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "python-dict-to-json", |a: Args| {
            gizza_ai_python_dict_to_json_core::convert(&a.input, &a.indent, a.sort_keys, a.ensure_ascii)
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Python dict or list literal to convert, e.g. as printed by print(obj) or repr(obj): {'name': 'Ann', 'active': True, 'tags': None}. Accepts single quotes, True/False/None, tuples and sets, trailing commas, # comments, 0x/0o/0b ints and 1_000 separators." },
                    "indent": { "type": "string", "enum": ["2", "4", "tab", "minify"], "default": "2", "description": "Output formatting: '2' or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line. Default 2." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Sort every object's keys alphabetically (like Python json.dumps(sort_keys=True)). Default false keeps the original key order." },
                    "ensure_ascii": { "type": "boolean", "default": false, "description": "Escape every non-ASCII character as \\uXXXX (Python json.dumps default). Default false emits readable UTF-8 verbatim." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
