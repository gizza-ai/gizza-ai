//! gizza-ai/keyvalue-text-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    custom_separator: String,
    #[serde(default = "default_structure")]
    structure: String,
    #[serde(default = "default_duplicates")]
    duplicates: String,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    unquote: bool,
    #[serde(default = "default_comments")]
    comment_prefixes: String,
    #[serde(default)]
    infer_types: bool,
    #[serde(default = "default_key_case")]
    key_case: String,
    #[serde(default = "default_unmatched")]
    unmatched: String,
    #[serde(default = "default_indent")]
    indent: f64,
}

fn default_separator() -> String {
    "auto".into()
}
fn default_structure() -> String {
    "object".into()
}
fn default_duplicates() -> String {
    "group".into()
}
fn default_true() -> bool {
    true
}
fn default_comments() -> String {
    "#,;,//".into()
}
fn default_key_case() -> String {
    "as-is".into()
}
fn default_unmatched() -> String {
    "skip".into()
}
fn default_indent() -> f64 {
    2.0
}

#[derive(Serialize)]
struct Resp {
    json: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Freeform text containing key-value lines such as 'Name: Ada' or 'role = engineer'. Lines without the selected separator are skipped by default; paste up to 10,000 lines."))
        .param(Param::enumv("separator", ["auto", "colon", "equals", "tab", "pipe", "custom"]).default("auto").describe("Separator used to split each line. 'auto' (default) uses whichever of ':' or '=' appears first on each line. Pick colon, equals, tab, pipe, or custom for stricter parsing."))
        .param(Param::string("custom_separator").default("").describe("Separator string used only when separator=custom, for example '->' or ' - '. Leave empty for the built-in separator modes."))
        .param(Param::enumv("structure", ["object", "records", "pairs"]).default("object").describe("Output shape. object returns one flat JSON object; records returns an array split on blank lines; pairs returns ordered {key,value,line} entries without folding duplicates."))
        .param(Param::enumv("duplicates", ["group", "last", "first", "error"]).default("group").describe("Duplicate-key policy inside object/record output. group (default) collects repeated keys into arrays, last overwrites, first keeps the first value, error fails on the repeated key."))
        .param(Param::boolean("trim").default(true).describe("Trim whitespace around keys and values before parsing. Default true."))
        .param(Param::boolean("unquote").default(true).describe("Remove one matching pair of single or double quotes around a value before optional type inference. Default true."))
        .param(Param::string("comment_prefixes").default("#,;,//").describe("Comma-separated line comment prefixes to skip after leading whitespace, for example '#,;,//'. Set empty to treat comment-looking lines as data."))
        .param(Param::boolean("infer_types").default(false).describe("Infer unquoted JSON booleans, nulls and numbers. Default false keeps values as strings so IDs like 0042 stay safe."))
        .param(Param::enumv("key_case", ["as-is", "lower", "snake"]).default("as-is").describe("Key normalization. as-is keeps the key text, lower lowercases it, snake converts punctuation and spaces to underscores."))
        .param(Param::enumv("unmatched", ["skip", "error"]).default("skip").describe("What to do with nonblank lines that do not contain the selected separator. skip is tolerant for freeform text; error names the offending line."))
        .param(Param::number("indent").min(0.0).max(8.0).default(2.0).describe("JSON indentation in spaces, from 0 for minified output to 8. Default 2."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/keyvalue-text-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse key-value text into JSON",
    skill(
        description = "Parse freeform key-value text into structured JSON. Accepts lines split by colon, equals, tab, pipe, or a custom separator; skips prose/comment lines by default; can split blank-line-separated records, preserve ordered pairs with line numbers, group or reject duplicate keys, normalize keys, unquote values, and optionally infer JSON booleans/nulls/numbers. Returns a JSON string suitable for copy/paste or downstream automation.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "keyvalue-text-parser", |a: Args| {
            let json = gizza_ai_keyvalue_text_parser_core::parse_text(
                &a.input,
                &a.separator,
                &a.custom_separator,
                &a.structure,
                &a.duplicates,
                a.trim,
                a.unquote,
                &a.comment_prefixes,
                a.infer_types,
                &a.key_case,
                &a.unmatched,
                a.indent,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { json })
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
            r##"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Freeform text containing key-value lines such as 'Name: Ada' or 'role = engineer'. Lines without the selected separator are skipped by default; paste up to 10,000 lines." },
                    "separator": { "type": "string", "enum": ["auto","colon","equals","tab","pipe","custom"], "default": "auto", "description": "Separator used to split each line. 'auto' (default) uses whichever of ':' or '=' appears first on each line. Pick colon, equals, tab, pipe, or custom for stricter parsing." },
                    "custom_separator": { "type": "string", "default": "", "description": "Separator string used only when separator=custom, for example '->' or ' - '. Leave empty for the built-in separator modes." },
                    "structure": { "type": "string", "enum": ["object","records","pairs"], "default": "object", "description": "Output shape. object returns one flat JSON object; records returns an array split on blank lines; pairs returns ordered {key,value,line} entries without folding duplicates." },
                    "duplicates": { "type": "string", "enum": ["group","last","first","error"], "default": "group", "description": "Duplicate-key policy inside object/record output. group (default) collects repeated keys into arrays, last overwrites, first keeps the first value, error fails on the repeated key." },
                    "trim": { "type": "boolean", "default": true, "description": "Trim whitespace around keys and values before parsing. Default true." },
                    "unquote": { "type": "boolean", "default": true, "description": "Remove one matching pair of single or double quotes around a value before optional type inference. Default true." },
                    "comment_prefixes": { "type": "string", "default": "#,;,//", "description": "Comma-separated line comment prefixes to skip after leading whitespace, for example '#,;,//'. Set empty to treat comment-looking lines as data." },
                    "infer_types": { "type": "boolean", "default": false, "description": "Infer unquoted JSON booleans, nulls and numbers. Default false keeps values as strings so IDs like 0042 stay safe." },
                    "key_case": { "type": "string", "enum": ["as-is","lower","snake"], "default": "as-is", "description": "Key normalization. as-is keeps the key text, lower lowercases it, snake converts punctuation and spaces to underscores." },
                    "unmatched": { "type": "string", "enum": ["skip","error"], "default": "skip", "description": "What to do with nonblank lines that do not contain the selected separator. skip is tolerant for freeform text; error names the offending line." },
                    "indent": { "type": "number", "minimum": 0, "maximum": 8, "default": 2.0, "description": "JSON indentation in spaces, from 0 for minified output to 8. Default 2." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
