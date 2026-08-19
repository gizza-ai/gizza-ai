//! gizza-ai/json-transform-rules — reshape JSON with declarative match/transform
//! rules, as a chat skill block on the shared tool abstraction.
//!
//! Thin wrapper around `gizza-ai-json-transform-rules-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    rules: String,
    #[serde(default)]
    each: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    on_missing: String,
    #[serde(default)]
    array_mode: String,
    #[serde(default = "default_pretty")]
    pretty: bool,
    #[serde(default = "default_indent")]
    indent: u32,
    #[serde(default)]
    output: String,
}

fn default_pretty() -> bool {
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
                .describe("The source JSON document to reshape, e.g. {\"user\":{\"id\":7,\"name\":\"Ada\"}}. Any JSON value works (object, array, or scalar). Maximum 5000000 bytes."),
        )
        .param(
            Param::string("rules")
                .required()
                .describe("The mapping rules, in any of three forms. (1) Shorthand lines, one per line: 'target = selector' (e.g. id = $.user.id), '-path' to drop a path in merge mode, '#'/'//' comments, and a quoted or JSON literal right-hand side for a constant (source = \"import\"). (2) A JSON object of target → selector: {\"id\": \"$.user.id\"}. (3) A JSON array of rule objects, the full form: {\"target\":\"total\",\"source\":\"$..price\",\"transform\":\"sum\",\"default\":0,\"when\":\"$.vip\",\"separator\":\", \",\"op\":\"set\"|\"remove\"}. Selectors are a JSONPath subset: $, .key, [\"key\"], [0], [*], .* and ..key (recursive descent). Targets are dotted/bracketed output paths (user.profile.email, items[0].id, tags[] to append) that create the objects and arrays they need. Transforms: upper, lower, trim, number, string, boolean, length, sort, reverse, unique, flatten, keys, values, count, sum, min, max, avg, first, last, join — a list transform applied to a lone array match reshapes that array's elements. A trailing '?' on the target ('tier ?= \"free\"') writes only when that output path is still absent or null. Maximum 500 rules."),
        )
        .param(
            Param::string("each")
                .default("")
                .describe("Optional selector that fans the rules out over many items: when set, every rule runs once per matched value and the result is an array of the per-item outputs (selectors inside the rules are then relative to the item). Example: $.automobiles[*]. Blank runs the rules once against the whole document. Maximum 50000 items."),
        )
        .param(
            Param::enumv("mode", ["build", "merge"])
                .default("build")
                .describe("What the output starts from. 'build' (default) starts from an empty object, so the output contains ONLY what the rules write. 'merge' starts from a copy of the input, so rules override individual paths and '-path' / op=remove rules delete them — use it for redaction or patching."),
        )
        .param(
            Param::enumv("on_missing", ["skip", "null", "error"])
                .default("skip")
                .describe("What to do when a rule's selector matches nothing and the rule has no 'default'. 'skip' (default) omits the target key entirely, 'null' writes null, 'error' fails with the offending target and selector. A rule's own 'default' always wins over this setting."),
        )
        .param(
            Param::enumv("array_mode", ["auto", "always", "first"])
                .default("auto")
                .describe("How a selector that matches several values is shaped. 'auto' (default): one match becomes the value itself, several become an array. 'always': always an array, and a rule that matched nothing writes an empty array. 'first': keep only the first match. Aggregate transforms (count, sum, min, max, avg, first, last, join) always produce a single value first."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the JSON output over multiple lines (default true). Set false for compact single-line JSON."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(8.0)
                .describe("Number of spaces per indent level when 'pretty' is on (default 2, maximum 8). Ignored when 'pretty' is false."),
        )
        .param(
            Param::enumv("output", ["json", "report"])
                .default("json")
                .describe("What to return. 'json' (default) is the reshaped document. 'report' is a plain-text trace of each rule — its target, source, transform, how many values it matched and how often it wrote, plus a list of rules that never produced anything — for debugging a mapping that came out empty."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonTransformRules;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-transform-rules",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reshape JSON into a new structure with declarative target = selector mapping rules.",
    skill(
        description = "Reshape a JSON document into a new structure using declarative mapping rules instead of an expression language. Each rule names an output 'target' path and where its value comes from — a JSONPath-subset 'source' selector ($, .key, [\"key\"], [0], [*], .*, ..key) or a literal 'value' — plus optional 'default', 'transform' (upper, lower, trim, number, string, boolean, length, sort, unique, count, sum, min, max, avg, first, last, join), 'separator', 'when' guard and op=remove. Rules are written as 'target = selector' shorthand lines, a target → selector JSON object, or a JSON array of rule objects. 'each' fans the rules out over every item a selector matches (one output object per item); 'mode=merge' starts from the input so rules patch or redact it instead of building a fresh object; 'on_missing' and 'array_mode' control unmatched selectors and multi-value matches; 'output=report' explains what each rule matched.",
        parameters = schema_json()
    ),
)]
impl JsonTransformRules {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-transform-rules", |a: Args| {
            gizza_ai_json_transform_rules_core::transform(
                &a.json,
                &a.rules,
                &a.each,
                &a.mode,
                &a.on_missing,
                &a.array_mode,
                a.pretty,
                a.indent as usize,
                &a.output,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The source JSON document to reshape, e.g. {\"user\":{\"id\":7,\"name\":\"Ada\"}}. Any JSON value works (object, array, or scalar). Maximum 5000000 bytes." },
                    "rules": { "type": "string", "description": "The mapping rules, in any of three forms. (1) Shorthand lines, one per line: 'target = selector' (e.g. id = $.user.id), '-path' to drop a path in merge mode, '#'/'//' comments, and a quoted or JSON literal right-hand side for a constant (source = \"import\"). (2) A JSON object of target → selector: {\"id\": \"$.user.id\"}. (3) A JSON array of rule objects, the full form: {\"target\":\"total\",\"source\":\"$..price\",\"transform\":\"sum\",\"default\":0,\"when\":\"$.vip\",\"separator\":\", \",\"op\":\"set\"|\"remove\"}. Selectors are a JSONPath subset: $, .key, [\"key\"], [0], [*], .* and ..key (recursive descent). Targets are dotted/bracketed output paths (user.profile.email, items[0].id, tags[] to append) that create the objects and arrays they need. Transforms: upper, lower, trim, number, string, boolean, length, sort, reverse, unique, flatten, keys, values, count, sum, min, max, avg, first, last, join — a list transform applied to a lone array match reshapes that array's elements. A trailing '?' on the target ('tier ?= \"free\"') writes only when that output path is still absent or null. Maximum 500 rules." },
                    "each": { "type": "string", "default": "", "description": "Optional selector that fans the rules out over many items: when set, every rule runs once per matched value and the result is an array of the per-item outputs (selectors inside the rules are then relative to the item). Example: $.automobiles[*]. Blank runs the rules once against the whole document. Maximum 50000 items." },
                    "mode": { "type": "string", "enum": ["build", "merge"], "default": "build", "description": "What the output starts from. 'build' (default) starts from an empty object, so the output contains ONLY what the rules write. 'merge' starts from a copy of the input, so rules override individual paths and '-path' / op=remove rules delete them — use it for redaction or patching." },
                    "on_missing": { "type": "string", "enum": ["skip", "null", "error"], "default": "skip", "description": "What to do when a rule's selector matches nothing and the rule has no 'default'. 'skip' (default) omits the target key entirely, 'null' writes null, 'error' fails with the offending target and selector. A rule's own 'default' always wins over this setting." },
                    "array_mode": { "type": "string", "enum": ["auto", "always", "first"], "default": "auto", "description": "How a selector that matches several values is shaped. 'auto' (default): one match becomes the value itself, several become an array. 'always': always an array, and a rule that matched nothing writes an empty array. 'first': keep only the first match. Aggregate transforms (count, sum, min, max, avg, first, last, join) always produce a single value first." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the JSON output over multiple lines (default true). Set false for compact single-line JSON." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Number of spaces per indent level when 'pretty' is on (default 2, maximum 8). Ignored when 'pretty' is false." },
                    "output": { "type": "string", "enum": ["json", "report"], "default": "json", "description": "What to return. 'json' (default) is the reshaped document. 'report' is a plain-text trace of each rule — its target, source, transform, how many values it matched and how often it wrote, plus a list of rules that never produced anything — for debugging a mapping that came out empty." }
                },
                "required": ["json", "rules"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
