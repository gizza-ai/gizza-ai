//! gizza-ai/json-dedupe-array — remove duplicate ELEMENTS from a JSON array,
//! either by whole-element structural equality or by chosen key fields.
//! Distinct from `jsonl-deduplicator` (NDJSON, one value per line) and from
//! `json-sort` (which reorders object keys). Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_dedupe_array_core::{dedupe, Keep, Options, OutputKind};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default)]
    keys: String,
    #[serde(default)]
    root: String,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_keep() -> String {
    "first".into()
}
fn default_output() -> String {
    "unique".into()
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON text to de-duplicate — a top-level array (of objects, strings, numbers, or nested values), or an object that contains one (see 'root')."),
        )
        .param(
            Param::string("keys")
                .default("")
                .describe("Comma-separated field(s) to compare on, e.g. \"id\" or \"user.email,country\" (dot-notation reaches nested fields; a numeric segment like \"tags.0\" indexes an array). Leave blank to compare WHOLE elements structurally, nested values included. Elements missing a listed field share one 'absent' group."),
        )
        .param(
            Param::string("root")
                .default("")
                .describe("Dot-path to the array when it is nested inside a wrapper object, e.g. \"data.items\" (a numeric segment indexes an array). Leave blank when the whole input is the array. The wrapper is kept in the output."),
        )
        .param(
            Param::enumv("keep", ["first", "last"])
                .default("first")
                .describe("Which occurrence of a duplicated element survives: 'first' (default) or 'last'. The survivor keeps that occurrence's position; the array's original order is otherwise preserved."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare case-insensitively so \"A@X.com\" and \"a@x.com\" collapse. Applies to string values and to field names. Off by default."),
        )
        .param(
            Param::enumv("output", ["unique", "duplicates", "report"])
                .default("unique")
                .describe("What to return: 'unique' (default) the de-duplicated array, 'duplicates' only the elements that were removed, or 'report' a JSON summary with total/unique/removed counts plus each duplicate group's 0-based indexes and kept element."),
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
        root: a.root.clone(),
        keep: Keep::parse(&a.keep),
        ignore_case: a.ignore_case,
        output: OutputKind::parse(&a.output),
        indent: a.indent as usize,
    }
}

#[cfg(target_arch = "wasm32")]
struct JsonDedupeArray;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-dedupe-array",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate elements from a JSON array, whole-value or by key fields",
    skill(
        description = "Remove duplicate ELEMENTS from a JSON array and return the de-duplicated array. By default elements are compared whole and structurally — nested objects and arrays included, and object key ORDER is ignored when comparing but preserved in the output, so {\"a\":1,\"b\":2} and {\"b\":2,\"a\":1} are duplicates. Set keys to a comma-separated list like \"id\" or \"user.email,country\" to compare only those fields (dot-notation reaches nested fields; a numeric segment indexes an array). Set root to a dot-path such as \"data.items\" when the array is nested inside a wrapper object; the wrapper is kept. keep chooses which occurrence survives, 'first' (default) or 'last'; original order is preserved either way. ignore_case=true compares case-insensitively. output selects 'unique' (default, the de-duplicated array), 'duplicates' (only the removed elements) or 'report' (total/unique/removed counts plus each duplicate group's indexes). indent is spaces per level (1-8, default 2), or 0 to minify. Numbers compare by value, so 2 and 2.0 are duplicates, and an absent field never matches an explicit null. Up to 200000 elements. For NDJSON/JSONL (one JSON value per line) use jsonl-deduplicator instead. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonDedupeArray {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-dedupe-array", |a: Args| {
            let opts = build_options(&a);
            dedupe(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json":        { "type": "string", "description": "The JSON text to de-duplicate — a top-level array (of objects, strings, numbers, or nested values), or an object that contains one (see 'root')." },
                    "keys":        { "type": "string", "default": "", "description": "Comma-separated field(s) to compare on, e.g. \"id\" or \"user.email,country\" (dot-notation reaches nested fields; a numeric segment like \"tags.0\" indexes an array). Leave blank to compare WHOLE elements structurally, nested values included. Elements missing a listed field share one 'absent' group." },
                    "root":        { "type": "string", "default": "", "description": "Dot-path to the array when it is nested inside a wrapper object, e.g. \"data.items\" (a numeric segment indexes an array). Leave blank when the whole input is the array. The wrapper is kept in the output." },
                    "keep":        { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which occurrence of a duplicated element survives: 'first' (default) or 'last'. The survivor keeps that occurrence's position; the array's original order is otherwise preserved." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare case-insensitively so \"A@X.com\" and \"a@x.com\" collapse. Applies to string values and to field names. Off by default." },
                    "output":      { "type": "string", "enum": ["unique", "duplicates", "report"], "default": "unique", "description": "What to return: 'unique' (default) the de-duplicated array, 'duplicates' only the elements that were removed, or 'report' a JSON summary with total/unique/removed counts plus each duplicate group's 0-based indexes and kept element." },
                    "indent":      { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8) in the output. Use 0 to minify to a single compact line. Default 2." }
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
