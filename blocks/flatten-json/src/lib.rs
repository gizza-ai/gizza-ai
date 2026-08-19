//! gizza-ai/flatten-json — flatten a nested JSON document into a single-level
//! map of path -> value, and rebuild it back. Thin chat-skill wrapper around
//! `gizza-ai-flatten-json-core`. The chat schema is single-sourced from
//! `descriptor()` (shared shape across chat + CLI); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default)]
    direction: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    array_notation: String,
    #[serde(default)]
    max_depth: u32,
    #[serde(default = "default_true")]
    flatten_arrays: bool,
    #[serde(default = "default_true")]
    preserve_empty: bool,
    #[serde(default)]
    key_case: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default = "default_indent")]
    indent: u32,
}

fn default_separator() -> String {
    ".".to_string()
}
fn default_true() -> bool {
    true
}
fn default_indent() -> u32 {
    2
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to convert. Flattening accepts an object or an array, e.g. {\"user\":{\"name\":\"Ada\",\"tags\":[\"a\"]}}; unflattening accepts a flat object of path -> value, e.g. {\"user.name\":\"Ada\"}. Capped at 5 MB, 100 levels of nesting and 200000 keys."),
        )
        .param(
            Param::enumv("direction", ["flatten", "unflatten", "auto"])
                .default("flatten")
                .describe("Which way to convert. 'flatten' (default) turns nested JSON into path -> value pairs; 'unflatten' rebuilds nested JSON from those pairs; 'auto' unflattens only when the input is a one-level object whose keys already look like paths, and flattens otherwise."),
        )
        .param(
            Param::string("separator")
                .default(".")
                .describe("String that joins object keys in a path. Default '.' (user.name); '_' gives user_name and '/' gives user/name. Any 1-8 characters except '[' and ']'. If a key in your document already contains the separator the run fails rather than silently merging two paths — pick another separator."),
        )
        .param(
            Param::enumv("array_notation", ["bracket", "separator"])
                .default("bracket")
                .describe("How array elements appear in a path. 'bracket' (default) writes tags[0]; 'separator' writes tags.0 using the separator. This also decides how a path is read back: with 'separator' an all-digit segment rebuilds an array, with 'bracket' it stays an object key literally named \"0\"."),
        )
        .param(
            Param::integer("max_depth")
                .default(0)
                .min(0.0)
                .max(100.0)
                .describe("How many levels to flatten. 0 (default) is unlimited. With 2, {\"a\":{\"b\":{\"c\":1}}} becomes {\"a.b\":{\"c\":1}} — anything deeper is left as a nested JSON value. Flatten only."),
        )
        .param(
            Param::boolean("flatten_arrays")
                .default(true)
                .describe("Expand arrays into indexed paths (default true). Set false to keep every array whole as a JSON value while still flattening objects — useful when a list is one logical cell. Flatten only."),
        )
        .param(
            Param::boolean("preserve_empty")
                .default(true)
                .describe("Keep empty objects and arrays as {} / [] leaf entries (default true), so the key survives and the document round-trips. Set false to drop those keys entirely. Flatten only."),
        )
        .param(
            Param::enumv("key_case", ["preserve", "upper", "lower"])
                .default("preserve")
                .describe("Case of the generated paths: 'preserve' (default) keeps the source keys, 'upper' gives ENV-style keys (with separator='_': DB_HOST), 'lower' gives SQL-style column keys. Upper/lower are lossy — the original key case cannot be recovered by unflattening. Flatten only."),
        )
        .param(
            Param::enumv("output", ["json", "pairs", "csv", "paths"])
                .default("json")
                .describe("Shape of the flattened result: 'json' (default) a JSON object of path -> value; 'pairs' one path=value line each (strings unquoted, so db.host=localhost); 'csv' a two-column key,value CSV with a header row for spreadsheets; 'paths' just the paths, one per line. Flatten only — unflattening always returns nested JSON, so leave this at json."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the JSON output (default true). Set false for one compact line. Ignored by the pairs/csv/keys outputs."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(8.0)
                .describe("Spaces per indent level when pretty=true. Default 2; 0 is the same as pretty=false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FlattenJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/flatten-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten nested JSON into dot/bracket key paths, and unflatten it back.",
    skill(
        description = "Flatten a nested JSON document into a single-level map of path -> value, and rebuild the nested document from those paths. direction='flatten' (default) turns {\"user\":{\"name\":\"Ada\",\"tags\":[\"a\",\"b\"]}} into {\"user.name\":\"Ada\",\"user.tags[0]\":\"a\",\"user.tags[1]\":\"b\"}; direction='unflatten' reverses it; 'auto' picks the direction from the input shape. separator sets the key joiner ('.' default, '_' or '/' also common) and array_notation chooses tags[0] (bracket, default) or tags.0 (separator) for array elements — both round-trip. max_depth flattens only the first N levels, flatten_arrays=false keeps arrays whole, preserve_empty=true keeps {} / [] keys, and key_case makes ENV-style (upper) or SQL-style (lower) keys. output returns the flat result as JSON, path=value pairs, a two-column key,value CSV for spreadsheets, or just the path list. Runs locally — the document never leaves the device.",
        parameters = schema_json()
    ),
)]
impl FlattenJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "flatten-json", |a: Args| {
            gizza_ai_flatten_json_core::convert(
                &a.json,
                &a.direction,
                &a.separator,
                &a.array_notation,
                a.max_depth as usize,
                a.flatten_arrays,
                a.preserve_empty,
                &a.key_case,
                &a.output,
                a.pretty,
                a.indent as usize,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON document to convert. Flattening accepts an object or an array, e.g. {\"user\":{\"name\":\"Ada\",\"tags\":[\"a\"]}}; unflattening accepts a flat object of path -> value, e.g. {\"user.name\":\"Ada\"}. Capped at 5 MB, 100 levels of nesting and 200000 keys." },
                    "direction": { "type": "string", "enum": ["flatten", "unflatten", "auto"], "default": "flatten", "description": "Which way to convert. 'flatten' (default) turns nested JSON into path -> value pairs; 'unflatten' rebuilds nested JSON from those pairs; 'auto' unflattens only when the input is a one-level object whose keys already look like paths, and flattens otherwise." },
                    "separator": { "type": "string", "default": ".", "description": "String that joins object keys in a path. Default '.' (user.name); '_' gives user_name and '/' gives user/name. Any 1-8 characters except '[' and ']'. If a key in your document already contains the separator the run fails rather than silently merging two paths — pick another separator." },
                    "array_notation": { "type": "string", "enum": ["bracket", "separator"], "default": "bracket", "description": "How array elements appear in a path. 'bracket' (default) writes tags[0]; 'separator' writes tags.0 using the separator. This also decides how a path is read back: with 'separator' an all-digit segment rebuilds an array, with 'bracket' it stays an object key literally named \"0\"." },
                    "max_depth": { "type": "integer", "default": 0, "minimum": 0, "maximum": 100, "description": "How many levels to flatten. 0 (default) is unlimited. With 2, {\"a\":{\"b\":{\"c\":1}}} becomes {\"a.b\":{\"c\":1}} — anything deeper is left as a nested JSON value. Flatten only." },
                    "flatten_arrays": { "type": "boolean", "default": true, "description": "Expand arrays into indexed paths (default true). Set false to keep every array whole as a JSON value while still flattening objects — useful when a list is one logical cell. Flatten only." },
                    "preserve_empty": { "type": "boolean", "default": true, "description": "Keep empty objects and arrays as {} / [] leaf entries (default true), so the key survives and the document round-trips. Set false to drop those keys entirely. Flatten only." },
                    "key_case": { "type": "string", "enum": ["preserve", "upper", "lower"], "default": "preserve", "description": "Case of the generated paths: 'preserve' (default) keeps the source keys, 'upper' gives ENV-style keys (with separator='_': DB_HOST), 'lower' gives SQL-style column keys. Upper/lower are lossy — the original key case cannot be recovered by unflattening. Flatten only." },
                    "output": { "type": "string", "enum": ["json", "pairs", "csv", "paths"], "default": "json", "description": "Shape of the flattened result: 'json' (default) a JSON object of path -> value; 'pairs' one path=value line each (strings unquoted, so db.host=localhost); 'csv' a two-column key,value CSV with a header row for spreadsheets; 'paths' just the paths, one per line. Flatten only — unflattening always returns nested JSON, so leave this at json." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the JSON output (default true). Set false for one compact line. Ignored by the pairs/csv/keys outputs." },
                    "indent": { "type": "integer", "default": 2, "minimum": 0, "maximum": 8, "description": "Spaces per indent level when pretty=true. Default 2; 0 is the same as pretty=false." }
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
    fn args_default_to_the_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"json":"{}"}"#).unwrap();
        assert_eq!(a.direction, "");
        assert_eq!(a.separator, ".");
        assert_eq!(a.max_depth, 0);
        assert!(a.flatten_arrays && a.preserve_empty && a.pretty);
        assert_eq!(a.indent, 2);
    }
}
