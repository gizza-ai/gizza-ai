//! gizza-ai/ini-json-converter — chat skill block on the shared tool abstraction.
//!
//! Converts INI / `.conf` text to JSON and back, and edits a single key in place
//! (`get` / `set` / `delete`) without disturbing comments, blank lines or key
//! order. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    section: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    detect_types: bool,
    #[serde(default = "default_true")]
    inline_comments: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default)]
    indent: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The config text to convert or edit: INI / .conf text ('[section]' headers, 'key = value' or 'key: value' pairs, ';' and '#' comments, keys above the first header) for every mode except json_to_ini, or a JSON object for json_to_ini."),
        )
        .param(
            Param::enumv("mode", ["auto", "ini_to_json", "json_to_ini", "get", "set", "delete"])
                .default("auto")
                .describe("What to do. 'auto' (default) converts JSON to INI when the input starts with '{' and INI to JSON otherwise; 'ini_to_json' and 'json_to_ini' force a direction; 'get' returns one key's value (or a whole section as JSON); 'set' writes `value` to one key; 'delete' removes one key, or a whole section when only `section` is given. get/set/delete need INI input and return the edited file with its comments, blank lines and key order intact."),
        )
        .param(
            Param::string("section")
                .describe("Section name to address for get/set/delete, without the brackets (e.g. 'database'). Leave blank to address the section-less keys at the top of the file, or to let a dotted `key` name the section. Ignored by the two conversion modes."),
        )
        .param(
            Param::string("key")
                .describe("Key to address for get/set/delete (e.g. 'port'). When `section` is blank, a dotted key is split at the first dot, so key='database.port' means key 'port' in section '[database]'. Leave blank with a `section` to dump that section as JSON (get) or remove the whole section (delete)."),
        )
        .param(
            Param::string("value")
                .describe("The new value to write in mode='set'. Written verbatim, so quotes, spaces and '=' inside the value are kept as typed; an empty string produces 'key = '. Ignored by every other mode."),
        )
        .param(
            Param::boolean("detect_types")
                .default(false)
                .describe("When true, INI values become real JSON types: true/false/yes/no/on/off → boolean, integers and decimals → number. Quoted values always stay strings. Default false, so every value stays a string and the text is preserved exactly."),
        )
        .param(
            Param::boolean("inline_comments")
                .default(true)
                .describe("When true (default), a ';' or '#' preceded by whitespace ends the value, so 'port = 8080 ; listen port' converts to '8080' and a set keeps the trailing note. Set false to treat the whole rest of the line as the value."),
        )
        .param(
            Param::enumv("delimiter", ["equals_spaced", "equals", "colon"])
                .default("equals_spaced")
                .describe("Separator for INI lines this tool writes (json_to_ini output and keys added by 'set'). 'equals_spaced' (default) writes 'key = value', 'equals' writes 'key=value', 'colon' writes 'key: value'. A key that already exists keeps the spacing it had."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("When true (default), JSON output is indented over multiple lines. Set false for a single compact line, which is easier to pipe into another command."),
        )
        .param(
            Param::enumv("indent", ["2", "4", "tab"])
                .default("2")
                .describe("One indent step of pretty JSON: '2' (default) two spaces, '4' four spaces, 'tab' a tab character. Ignored when pretty=false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IniJsonConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ini-json-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert INI/conf files to JSON and back, or get, set and delete a single key in place.",
    skill(
        description = "Convert INI / .conf config text to JSON and back, or read and edit a single setting. mode='auto' (default) picks the direction from the input; 'ini_to_json' and 'json_to_ini' force one. mode='get' returns one key's value, 'set' writes `value` to one key, and 'delete' removes a key — or a whole section when only `section` is given. The edit modes rewrite just the target line, so comments, blank lines, key order and the file's existing 'key=value' vs 'key = value' spacing all survive. Address a setting with `section` + `key`, or with a dotted key alone ('database.port'). Sections become nested JSON objects, keys above the first header stay at the root, a key repeated in one section becomes a JSON array, and a JSON array of scalars converts back to repeated key lines. Set detect_types=true to turn values into real JSON booleans and numbers, inline_comments=false to keep trailing ' ; note' text in the value, `delimiter` to choose the spacing of lines this tool writes, `indent` (2/4/tab) for the JSON indent step, and pretty=false for compact JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl IniJsonConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "ini-json-converter", |a: Args| {
            gizza_ai_ini_json_converter_core::convert(
                &a.input,
                &a.mode,
                &a.section,
                &a.key,
                &a.value,
                a.detect_types,
                a.inline_comments,
                &a.delimiter,
                a.pretty,
                &a.indent,
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
                    "input": { "type": "string", "description": "The config text to convert or edit: INI / .conf text ('[section]' headers, 'key = value' or 'key: value' pairs, ';' and '#' comments, keys above the first header) for every mode except json_to_ini, or a JSON object for json_to_ini." },
                    "mode": { "type": "string", "enum": ["auto", "ini_to_json", "json_to_ini", "get", "set", "delete"], "default": "auto", "description": "What to do. 'auto' (default) converts JSON to INI when the input starts with '{' and INI to JSON otherwise; 'ini_to_json' and 'json_to_ini' force a direction; 'get' returns one key's value (or a whole section as JSON); 'set' writes `value` to one key; 'delete' removes one key, or a whole section when only `section` is given. get/set/delete need INI input and return the edited file with its comments, blank lines and key order intact." },
                    "section": { "type": "string", "description": "Section name to address for get/set/delete, without the brackets (e.g. 'database'). Leave blank to address the section-less keys at the top of the file, or to let a dotted `key` name the section. Ignored by the two conversion modes." },
                    "key": { "type": "string", "description": "Key to address for get/set/delete (e.g. 'port'). When `section` is blank, a dotted key is split at the first dot, so key='database.port' means key 'port' in section '[database]'. Leave blank with a `section` to dump that section as JSON (get) or remove the whole section (delete)." },
                    "value": { "type": "string", "description": "The new value to write in mode='set'. Written verbatim, so quotes, spaces and '=' inside the value are kept as typed; an empty string produces 'key = '. Ignored by every other mode." },
                    "detect_types": { "type": "boolean", "default": false, "description": "When true, INI values become real JSON types: true/false/yes/no/on/off → boolean, integers and decimals → number. Quoted values always stay strings. Default false, so every value stays a string and the text is preserved exactly." },
                    "inline_comments": { "type": "boolean", "default": true, "description": "When true (default), a ';' or '#' preceded by whitespace ends the value, so 'port = 8080 ; listen port' converts to '8080' and a set keeps the trailing note. Set false to treat the whole rest of the line as the value." },
                    "delimiter": { "type": "string", "enum": ["equals_spaced", "equals", "colon"], "default": "equals_spaced", "description": "Separator for INI lines this tool writes (json_to_ini output and keys added by 'set'). 'equals_spaced' (default) writes 'key = value', 'equals' writes 'key=value', 'colon' writes 'key: value'. A key that already exists keeps the spacing it had." },
                    "pretty": { "type": "boolean", "default": true, "description": "When true (default), JSON output is indented over multiple lines. Set false for a single compact line, which is easier to pipe into another command." },
                    "indent": { "type": "string", "enum": ["2", "4", "tab"], "default": "2", "description": "One indent step of pretty JSON: '2' (default) two spaces, '4' four spaces, 'tab' a tab character. Ignored when pretty=false." }
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
