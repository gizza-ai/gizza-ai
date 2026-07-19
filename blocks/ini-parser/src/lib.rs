//! gizza-ai/ini-parser — chat skill block on the shared tool abstraction.
//!
//! Parses INI / `.conf` text (sections, key=value / key:value pairs, `;`/`#`
//! comments) into structured JSON and reports duplicate keys. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI + page);
//! `handle()` delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ini: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    duplicate_keys: String,
    #[serde(default)]
    detect_types: bool,
    #[serde(default)]
    comments: String,
    #[serde(default)]
    inline_comments: bool,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ini")
                .required()
                .describe("The INI / .conf text to parse. Sections are '[name]' headers; entries are 'key = value' or 'key: value'; lines starting with ';' or '#' are comments. Keys before the first section are 'global'."),
        )
        .param(
            Param::enumv("output", ["json", "flat", "report"])
                .default("json")
                .describe("Output shape. 'json' (default) is a nested config object (global keys at the root, each section a nested object); 'flat' is a flat map of dotted keys ('section.key'); 'report' adds a 'duplicates' list and a 'stats' block (sections/keys/comments/duplicates)."),
        )
        .param(
            Param::enumv("duplicate_keys", ["last", "first", "array", "error"])
                .default("last")
                .describe("How to handle a key repeated within one section. 'last' (default) keeps the last value; 'first' keeps the first; 'array' collects all values into an array; 'error' fails on any duplicate. Duplicates are always listed when output='report'."),
        )
        .param(
            Param::boolean("detect_types")
                .default(false)
                .describe("When true, convert unquoted values to JSON types: true/false/yes/no/on/off → boolean, integers and decimals → number. Quoted values always stay strings. Default false (every value stays a string, lossless)."),
        )
        .param(
            Param::enumv("comments", ["both", "semicolon", "hash"])
                .default("both")
                .describe("Which characters start a comment line. 'both' (default) treats ';' and '#' as comments; 'semicolon' only ';'; 'hash' only '#'. A line whose comment char is disabled is parsed as data."),
        )
        .param(
            Param::boolean("inline_comments")
                .default(false)
                .describe("When true, also strip a trailing comment from a value (a ';' or '#' preceded by whitespace, e.g. 'port = 8080 ; note'). Default false — trailing text is kept as part of the value."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IniParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ini-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse INI/conf files into structured JSON and report duplicate keys.",
    skill(
        description = "Parse INI / .conf text into structured JSON and report duplicate keys. Handles '[section]' headers (nested objects), 'key = value' and 'key: value' pairs, ';' and '#' comments, section-less 'global' keys, and quoted values. output='json' (default) is a nested config object, 'flat' is dotted keys, 'report' adds a duplicates list and stats. duplicate_keys controls repeated keys (last/first/array/error). Set detect_types=true to coerce booleans and numbers, inline_comments=true to strip trailing ' ; note' comments, and comments to choose which prefixes count.",
        parameters = schema_json()
    ),
)]
impl IniParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "ini-parser", |a: Args| {
            gizza_ai_ini_parser_core::parse(
                &a.ini,
                &a.output,
                &a.duplicate_keys,
                a.detect_types,
                &a.comments,
                a.inline_comments,
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
                    "ini": { "type": "string", "description": "The INI / .conf text to parse. Sections are '[name]' headers; entries are 'key = value' or 'key: value'; lines starting with ';' or '#' are comments. Keys before the first section are 'global'." },
                    "output": { "type": "string", "enum": ["json", "flat", "report"], "default": "json", "description": "Output shape. 'json' (default) is a nested config object (global keys at the root, each section a nested object); 'flat' is a flat map of dotted keys ('section.key'); 'report' adds a 'duplicates' list and a 'stats' block (sections/keys/comments/duplicates)." },
                    "duplicate_keys": { "type": "string", "enum": ["last", "first", "array", "error"], "default": "last", "description": "How to handle a key repeated within one section. 'last' (default) keeps the last value; 'first' keeps the first; 'array' collects all values into an array; 'error' fails on any duplicate. Duplicates are always listed when output='report'." },
                    "detect_types": { "type": "boolean", "default": false, "description": "When true, convert unquoted values to JSON types: true/false/yes/no/on/off → boolean, integers and decimals → number. Quoted values always stay strings. Default false (every value stays a string, lossless)." },
                    "comments": { "type": "string", "enum": ["both", "semicolon", "hash"], "default": "both", "description": "Which characters start a comment line. 'both' (default) treats ';' and '#' as comments; 'semicolon' only ';'; 'hash' only '#'. A line whose comment char is disabled is parsed as data." },
                    "inline_comments": { "type": "boolean", "default": false, "description": "When true, also strip a trailing comment from a value (a ';' or '#' preceded by whitespace, e.g. 'port = 8080 ; note'). Default false — trailing text is kept as part of the value." }
                },
                "required": ["ini"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
