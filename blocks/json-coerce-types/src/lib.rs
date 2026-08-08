//! gizza-ai/json-coerce-types — chat skill block on the shared tool abstraction.
//!
//! Walks a VALID JSON document and retypes loosely-typed string scalars —
//! `"42"` → `42`, `"true"` → `true`, `"null"` → `null` — at every depth. The
//! chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_true")]
    numbers: bool,
    #[serde(default = "default_true")]
    booleans: bool,
    #[serde(default = "default_true")]
    nulls: bool,
    #[serde(default)]
    bool_synonyms: bool,
    #[serde(default)]
    null_tokens: String,
    #[serde(default)]
    empty_strings: String,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    leading_zeros: String,
    #[serde(default)]
    thousands: bool,
    #[serde(default)]
    skip_keys: String,
    #[serde(default)]
    only_keys: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

fn default_indent() -> i64 {
    2
}

/// Single source for the chat schema (and CLI). Param order here is also the
/// page field order and the `web::run` argument order — keep the three in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The JSON document to retype. Must already be VALID JSON (object, array or bare scalar) — up to 5000000 bytes — e.g. '{\"age\": \"42\", \"active\": \"true\", \"note\": \"null\"}'. Key order is preserved and no key is ever added or removed."),
        )
        .param(
            Param::boolean("numbers")
                .default(true)
                .describe("Coerce numeric strings into JSON numbers: '\"42\"' becomes 42, '\"-3.5\"' becomes -3.5, '\"1e3\"' becomes 1000.0. Only strict JSON number shapes convert, so \"12px\", \"0x1f\", \".5\" and \"1.\" stay strings, and an integer too large for 64 bits stays a string rather than losing precision. Default true."),
        )
        .param(
            Param::boolean("booleans")
                .default(true)
                .describe("Coerce \"true\" and \"false\" (any capitalisation, e.g. \"TRUE\") into JSON booleans. Default true."),
        )
        .param(
            Param::boolean("nulls")
                .default(true)
                .describe("Coerce the string \"null\" (any capitalisation) into JSON null. Default true."),
        )
        .param(
            Param::boolean("bool_synonyms")
                .default(false)
                .describe("Also treat yes/no/on/off/y/n (any capitalisation) as booleans, so \"yes\" becomes true and \"off\" becomes false. Needs booleans=true. Default false, because these words are often real data."),
        )
        .param(
            Param::string("null_tokens")
                .default("")
                .describe("Comma-separated extra tokens that become null, e.g. 'NA,N/A,-,none'. Matched case-insensitively against the whole value. Blank (default) adds none beyond the literal \"null\"."),
        )
        .param(
            Param::enumv("empty_strings", ["keep", "null"])
                .default("keep")
                .describe("What an empty string value becomes. 'keep' (default) leaves \"\" untouched; 'null' turns it into null, which is what a blank spreadsheet or form field usually means."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from every string value before testing it, and keep the trimmed text when nothing coerces: \"  42  \" becomes 42 and \"  hi  \" becomes \"hi\". Default false."),
        )
        .param(
            Param::enumv("leading_zeros", ["keep", "coerce"])
                .default("keep")
                .describe("What to do with zero-padded numeric strings such as \"0005\" or the ZIP code \"02134\". 'keep' (default) leaves them as strings so codes and padded IDs survive; 'coerce' converts them anyway (\"0005\" becomes 5). A lone \"0\" always converts."),
        )
        .param(
            Param::boolean("thousands")
                .default(false)
                .describe("Accept ',' as a thousands separator, so \"1,234.5\" becomes 1234.5 and \"385,134\" becomes 385134. Only well-formed 3-digit groupings convert; \"1,23\" and \"12,3456\" stay strings. Default false."),
        )
        .param(
            Param::string("skip_keys")
                .default("")
                .describe("Comma-separated object keys to leave completely alone — the key's whole subtree is copied through untouched, e.g. 'zip,phone,id'. Exact, case-sensitive match on the key name at any depth. Blank (default) skips nothing."),
        )
        .param(
            Param::string("only_keys")
                .default("")
                .describe("Comma-separated object keys to restrict the sweep to — only values at, or nested under, these keys are considered, e.g. 'counts,scores'. Exact, case-sensitive match at any depth. Blank (default) sweeps the whole document."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Output indentation in spaces (0-8). Use 0 to minify onto one line. Default 2. Ignored when output='report'."),
        )
        .param(
            Param::enumv("output", ["json", "report"])
                .default("json")
                .describe("What to return. 'json' (default) is the retyped document; 'report' is a plain-text audit listing every coerced path with its before and after value (e.g. '$.user.age: \"42\" -> 42'), so you can review the damage before applying it."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-coerce-types",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Retype stringified JSON values into real numbers, booleans and nulls",
    skill(
        description = "Walk a VALID JSON document and coerce loosely-typed string scalars into native JSON types at every depth: \"42\" becomes 42, \"true\" becomes true, \"null\" becomes null. Made for CSV importers, form posts and older APIs that quote every scalar. numbers/booleans/nulls (all default true) switch each type off individually; bool_synonyms adds yes/no/on/off/y/n; null_tokens lists extra tokens that become null (e.g. 'NA,N/A,-'); empty_strings='null' turns \"\" into null; trim strips surrounding whitespace before testing; leading_zeros='keep' (default) protects \"0005\" and ZIP codes, 'coerce' converts them; thousands accepts \"1,234.5\"; skip_keys and only_keys scope the sweep by object key; indent sets output spacing (0 minifies); output='report' lists every coerced path with before -> after instead of returning the document. Nothing is ever deleted, key order is preserved, invalid JSON is rejected with its line/column, and integers too large for 64 bits stay strings rather than losing precision. Input is capped at 5000000 bytes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-coerce-types", |a: Args| {
            gizza_ai_json_coerce_types_core::run(
                &a.input,
                a.numbers,
                a.booleans,
                a.nulls,
                a.bool_synonyms,
                &a.null_tokens,
                &a.empty_strings,
                a.trim,
                &a.leading_zeros,
                a.thousands,
                &a.skip_keys,
                &a.only_keys,
                a.indent.max(0) as usize,
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
                    "input": { "type": "string", "description": "The JSON document to retype. Must already be VALID JSON (object, array or bare scalar) — up to 5000000 bytes — e.g. '{\"age\": \"42\", \"active\": \"true\", \"note\": \"null\"}'. Key order is preserved and no key is ever added or removed." },
                    "numbers": { "type": "boolean", "default": true, "description": "Coerce numeric strings into JSON numbers: '\"42\"' becomes 42, '\"-3.5\"' becomes -3.5, '\"1e3\"' becomes 1000.0. Only strict JSON number shapes convert, so \"12px\", \"0x1f\", \".5\" and \"1.\" stay strings, and an integer too large for 64 bits stays a string rather than losing precision. Default true." },
                    "booleans": { "type": "boolean", "default": true, "description": "Coerce \"true\" and \"false\" (any capitalisation, e.g. \"TRUE\") into JSON booleans. Default true." },
                    "nulls": { "type": "boolean", "default": true, "description": "Coerce the string \"null\" (any capitalisation) into JSON null. Default true." },
                    "bool_synonyms": { "type": "boolean", "default": false, "description": "Also treat yes/no/on/off/y/n (any capitalisation) as booleans, so \"yes\" becomes true and \"off\" becomes false. Needs booleans=true. Default false, because these words are often real data." },
                    "null_tokens": { "type": "string", "default": "", "description": "Comma-separated extra tokens that become null, e.g. 'NA,N/A,-,none'. Matched case-insensitively against the whole value. Blank (default) adds none beyond the literal \"null\"." },
                    "empty_strings": { "type": "string", "enum": ["keep", "null"], "default": "keep", "description": "What an empty string value becomes. 'keep' (default) leaves \"\" untouched; 'null' turns it into null, which is what a blank spreadsheet or form field usually means." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from every string value before testing it, and keep the trimmed text when nothing coerces: \"  42  \" becomes 42 and \"  hi  \" becomes \"hi\". Default false." },
                    "leading_zeros": { "type": "string", "enum": ["keep", "coerce"], "default": "keep", "description": "What to do with zero-padded numeric strings such as \"0005\" or the ZIP code \"02134\". 'keep' (default) leaves them as strings so codes and padded IDs survive; 'coerce' converts them anyway (\"0005\" becomes 5). A lone \"0\" always converts." },
                    "thousands": { "type": "boolean", "default": false, "description": "Accept ',' as a thousands separator, so \"1,234.5\" becomes 1234.5 and \"385,134\" becomes 385134. Only well-formed 3-digit groupings convert; \"1,23\" and \"12,3456\" stay strings. Default false." },
                    "skip_keys": { "type": "string", "default": "", "description": "Comma-separated object keys to leave completely alone — the key's whole subtree is copied through untouched, e.g. 'zip,phone,id'. Exact, case-sensitive match on the key name at any depth. Blank (default) skips nothing." },
                    "only_keys": { "type": "string", "default": "", "description": "Comma-separated object keys to restrict the sweep to — only values at, or nested under, these keys are considered, e.g. 'counts,scores'. Exact, case-sensitive match at any depth. Blank (default) sweeps the whole document." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Output indentation in spaces (0-8). Use 0 to minify onto one line. Default 2. Ignored when output='report'." },
                    "output": { "type": "string", "enum": ["json", "report"], "default": "json", "description": "What to return. 'json' (default) is the retyped document; 'report' is a plain-text audit listing every coerced path with its before and after value (e.g. '$.user.age: \"42\" -> 42'), so you can review the damage before applying it." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page form, the CLI and `web::run` all key off this order — a
    /// reordered descriptor silently shifts every page field one slot.
    #[test]
    fn param_order_matches_the_page_and_web_wrapper() {
        let d = descriptor();
        let names: Vec<&str> = d
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "input",
                "numbers",
                "booleans",
                "nulls",
                "bool_synonyms",
                "null_tokens",
                "empty_strings",
                "trim",
                "leading_zeros",
                "thousands",
                "skip_keys",
                "only_keys",
                "indent",
                "output",
            ]
        );
    }

    /// Every param must tell an LLM/CLI user what to pass.
    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(
                p.description.len() > 30,
                "param {} needs a real .describe()",
                p.name
            );
        }
    }
}
