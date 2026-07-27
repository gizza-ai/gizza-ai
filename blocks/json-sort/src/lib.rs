//! gizza-ai/json-sort — recursively sort JSON object keys (and, optionally,
//! array elements) into a stable, diff-friendly order, validating the input.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_sort_core::{sort, Options, Order};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    sort_arrays: bool,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_order() -> String {
    "asc".into()
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe("The JSON text whose object keys should be sorted."))
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Key sort direction: 'asc' (A→Z, default) or 'desc' (Z→A)."),
        )
        .param(
            Param::boolean("sort_arrays")
                .default(false)
                .describe("Also sort the elements inside every array (by their serialized value), not just object keys. Off by default so meaningful array order is preserved."),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(false)
                .describe("Compare keys case-insensitively so 'Name' sorts next to 'name'. Off by default, where uppercase letters sort before lowercase (codepoint order)."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(order: &str, sort_arrays: bool, case_insensitive: bool, indent: u64) -> Options {
    Options {
        order: Order::parse(order),
        sort_arrays,
        case_insensitive,
        indent: indent as usize,
    }
}

#[cfg(target_arch = "wasm32")]
struct JsonSort;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-sort",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recursively sort JSON object keys for stable, diff-friendly output",
    skill(
        description = "Recursively sort the object keys of a JSON document into a stable, diff-friendly order, validating it in the process (returns a line/column error if invalid). order is 'asc' (A→Z, default) or 'desc'; set case_insensitive=true to group keys ignoring case; set sort_arrays=true to also reorder array elements (off by default so meaningful array order is kept); indent is spaces per level (1-8, default 2), or 0 to minify. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonSort {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-sort", |a: Args| {
            let opts = build_options(&a.order, a.sort_arrays, a.case_insensitive, a.indent);
            sort(&a.json, opts).map_err(SkillError::InvalidArgs)
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
                    "json":             { "type": "string", "description": "The JSON text whose object keys should be sorted." },
                    "order":            { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Key sort direction: 'asc' (A→Z, default) or 'desc' (Z→A)." },
                    "sort_arrays":      { "type": "boolean", "default": false, "description": "Also sort the elements inside every array (by their serialized value), not just object keys. Off by default so meaningful array order is preserved." },
                    "case_insensitive": { "type": "boolean", "default": false, "description": "Compare keys case-insensitively so 'Name' sorts next to 'name'. Off by default, where uppercase letters sort before lowercase (codepoint order)." },
                    "indent":           { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
