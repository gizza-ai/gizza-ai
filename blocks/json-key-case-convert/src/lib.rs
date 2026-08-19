//! gizza-ai/json-key-case-convert — rewrite every JSON object key into one
//! naming convention (camelCase, PascalCase, snake_case, kebab-case,
//! SCREAMING_SNAKE_CASE) without touching a single value. Chat schema
//! single-sourced from descriptor() (which also drives the CLI); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_key_case_convert_core::{convert, parse_preserve_keys, Case, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_target_case")]
    target_case: String,
    #[serde(default = "default_true")]
    recurse: bool,
    #[serde(default)]
    preserve_keys: String,
    #[serde(default = "default_true")]
    preserve_prefix: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_target_case() -> String {
    "camel".into()
}
fn default_true() -> bool {
    true
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON text whose object keys should be renamed, e.g. {\"user_id\":1,\"profile_data\":{\"first_name\":\"ada\"}}. Values are never modified."),
        )
        .param(
            Param::enumv("target_case", ["camel", "pascal", "snake", "kebab", "constant"])
                .default("camel")
                .describe("Naming convention for every rewritten key: 'camel' (userId, default), 'pascal' (UserId), 'snake' (user_id), 'kebab' (user-id) or 'constant' (SCREAMING_SNAKE, USER_ID)."),
        )
        .param(
            Param::boolean("recurse")
                .default(true)
                .describe("Rename keys at every nesting level, including objects inside arrays. Default true. Set false to rename only the outermost object's keys (for a root array, the keys of the objects directly inside it)."),
        )
        .param(
            Param::string("preserve_keys")
                .default("")
                .describe("Comma-separated exact key names to leave untouched, case-sensitive, e.g. 'Content-Type,_id'. Use it for header names and for objects whose keys are data (ids, dates). Default: none."),
        )
        .param(
            Param::boolean("preserve_prefix")
                .default(true)
                .describe("Keep a key's leading sigils and convert only the rest, so '_id' stays '_id' and '$schema_url' becomes '$schemaUrl'. Default true; set false to strip them ('_id' becomes 'id')."),
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

fn build_options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        target: Case::parse(&a.target_case)?,
        recurse: a.recurse,
        preserve_keys: parse_preserve_keys(&a.preserve_keys),
        preserve_prefix: a.preserve_prefix,
        indent: a.indent as usize,
    })
}

#[cfg(target_arch = "wasm32")]
struct JsonKeyCaseConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-key-case-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rename every JSON key to camelCase, PascalCase, snake_case, kebab-case or SCREAMING_SNAKE_CASE",
    skill(
        description = "Rewrite every object key in a JSON document into one naming convention, leaving all values byte-identical. target_case is 'camel' (default), 'pascal', 'snake', 'kebab' or 'constant' (SCREAMING_SNAKE). Splitting is acronym-aware, so userID becomes user_id and HTTPResponse becomes httpResponse. Keys are renamed at every level including inside arrays unless recurse=false; leading sigils like _id and $schema survive unless preserve_prefix=false; preserve_keys lists exact key names to skip. indent is spaces per level (1-8, default 2) or 0 to minify. Invalid JSON is reported with line/column, and two keys that would collide after renaming are an error rather than a silent overwrite. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonKeyCaseConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-key-case-convert", |a: Args| {
            let opts = build_options(&a).map_err(SkillError::InvalidArgs)?;
            convert(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json":            { "type": "string", "description": "The JSON text whose object keys should be renamed, e.g. {\"user_id\":1,\"profile_data\":{\"first_name\":\"ada\"}}. Values are never modified." },
                    "target_case":     { "type": "string", "enum": ["camel", "pascal", "snake", "kebab", "constant"], "default": "camel", "description": "Naming convention for every rewritten key: 'camel' (userId, default), 'pascal' (UserId), 'snake' (user_id), 'kebab' (user-id) or 'constant' (SCREAMING_SNAKE, USER_ID)." },
                    "recurse":         { "type": "boolean", "default": true, "description": "Rename keys at every nesting level, including objects inside arrays. Default true. Set false to rename only the outermost object's keys (for a root array, the keys of the objects directly inside it)." },
                    "preserve_keys":   { "type": "string", "default": "", "description": "Comma-separated exact key names to leave untouched, case-sensitive, e.g. 'Content-Type,_id'. Use it for header names and for objects whose keys are data (ids, dates). Default: none." },
                    "preserve_prefix": { "type": "boolean", "default": true, "description": "Keep a key's leading sigils and convert only the rest, so '_id' stays '_id' and '$schema_url' becomes '$schemaUrl'. Default true; set false to strip them ('_id' becomes 'id')." },
                    "indent":          { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8) in the output. Use 0 to minify to a single compact line. Default 2." }
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
    fn build_options_rejects_unknown_case() {
        let a = Args {
            json: "{}".into(),
            target_case: "dromedary".into(),
            recurse: true,
            preserve_keys: String::new(),
            preserve_prefix: true,
            indent: 2,
        };
        assert!(build_options(&a).unwrap_err().contains("invalid target_case"));
    }
}
