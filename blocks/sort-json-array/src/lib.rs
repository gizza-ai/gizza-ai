//! gizza-ai/sort-json-array — sort a JSON array of objects by one or more fields
//! (comma-separated keys, nested dot-paths, per-key +/- direction). Distinct from
//! the `json-sort` tool, which reorders object KEYS rather than array ELEMENTS.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sort_json_array_core::{sort, Missing, Options, Order};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    keys: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default = "default_missing")]
    missing: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_order() -> String {
    "asc".into()
}
fn default_missing() -> String {
    "last".into()
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON text — a top-level array of objects — to sort by field value."),
        )
        .param(
            Param::string("keys")
                .required()
                .describe("Comma-separated field(s) to sort by. Use dot-notation for nested paths ('address.city') or an array index ('tags.0'). Prefix a key with '-' for descending or '+' for ascending to override 'order' per key, e.g. 'dept,-salary,name'."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Default direction for keys with no +/- prefix: 'asc' (smallest first, default) or 'desc' (largest first)."),
        )
        .param(
            Param::enumv("missing", ["last", "first"])
                .default("last")
                .describe("Where rows whose sort field is absent or JSON null go: 'last' (default) or 'first'. Placement is absolute — it does not flip with descending order."),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(false)
                .describe("Compare string values ignoring case so 'Banana' sorts next to 'apple'. Off by default, where uppercase letters sort before lowercase (codepoint order). Numbers always compare numerically."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level (1-8) in the output. Use 0 to minify to a single compact line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        keys: a.keys.clone(),
        order: Order::parse(&a.order),
        missing: Missing::parse(&a.missing),
        case_insensitive: a.case_insensitive,
        indent: a.indent as usize,
    }
}

#[cfg(target_arch = "wasm32")]
struct SortJsonArray;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sort-json-array",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort a JSON array of objects by one or more fields, ascending or descending",
    skill(
        description = "Sort a JSON array of objects by the value of one or more fields (the ORDER BY / _.orderBy operation), validating the input first. keys is a comma-separated list of fields; use dot-notation for nested paths ('address.city') or an array index ('tags.0'), and prefix a key with '-' (descending) or '+' (ascending) to override the per-key direction, e.g. 'dept,-salary,name'. order is the default direction 'asc' (default) or 'desc'; missing places rows whose field is absent or null 'last' (default) or 'first'; set case_insensitive=true to compare strings ignoring case (numbers always compare numerically); indent is spaces per level (1-8, default 2), or 0 to minify. This sorts array ELEMENTS by a field — to alphabetize object KEYS use json-sort instead. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SortJsonArray {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sort-json-array", |a: Args| {
            let opts = build_options(&a);
            sort(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json":             { "type": "string", "description": "The JSON text — a top-level array of objects — to sort by field value." },
                    "keys":             { "type": "string", "description": "Comma-separated field(s) to sort by. Use dot-notation for nested paths ('address.city') or an array index ('tags.0'). Prefix a key with '-' for descending or '+' for ascending to override 'order' per key, e.g. 'dept,-salary,name'." },
                    "order":            { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Default direction for keys with no +/- prefix: 'asc' (smallest first, default) or 'desc' (largest first)." },
                    "missing":          { "type": "string", "enum": ["last", "first"], "default": "last", "description": "Where rows whose sort field is absent or JSON null go: 'last' (default) or 'first'. Placement is absolute — it does not flip with descending order." },
                    "case_insensitive": { "type": "boolean", "default": false, "description": "Compare string values ignoring case so 'Banana' sorts next to 'apple'. Off by default, where uppercase letters sort before lowercase (codepoint order). Numbers always compare numerically." },
                    "indent":           { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8) in the output. Use 0 to minify to a single compact line. Default 2." }
                },
                "required": ["json", "keys"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
