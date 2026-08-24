//! gizza-ai/jmespath-query — evaluate a JMESPath expression against a JSON document
//! (pure-Rust `jmespath`, the reference implementation). Thin wrapper around the core;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jmespath_query_core::{json_kind, run_jmespath};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    expression: String,
    json: String,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default)]
    raw: bool,
}

#[derive(Serialize)]
struct Resp {
    /// The single JMESPath result, serialized as a JSON string (or as unquoted
    /// text when raw=true). An expression that matches nothing yields "null".
    result: String,
    /// The JSON type of the result: object, array, string, number, boolean, or null.
    kind: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("expression").required().describe("The JMESPath expression — the same syntax as the AWS CLI --query flag. Examples: 'people[*].name', \"people[?state == 'WA'].name\", 'people[?age > `30`]', 'sort_by(people, &age)[0]', 'people[0].{who: name, howOld: age}', 'length(items)'. Quote string literals with single quotes and JSON literals with backticks."))
        .param(Param::string("json").required().describe("The JSON document to query."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print (indent by 2 spaces) the JSON result. Default true; set false for compact single-line JSON."))
        .param(Param::boolean("raw").default(false).describe("Emit strings unquoted, like `aws --output text` or `jq -r`: a string result loses its quotes and a top-level array prints one element per line. Non-string results are unchanged. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JmespathQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jmespath-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Evaluate a JMESPath expression against JSON",
    skill(
        description = "Filter and reshape a JSON document with a JMESPath expression, using the pure-Rust reference engine. JMESPath is the query language behind the AWS CLI --query flag: it can project fields ('people[*].name'), filter ('people[?state == `WA`]', 'people[?age > `30`]'), slice ('items[:3]'), flatten nested arrays ('people[].skills[]'), pipe ('people[*].name | [0]'), construct new shapes with multiselect hashes/lists ('people[0].{who: name, howOld: age}'), and call built-ins (length, sort_by, max_by, min_by, join, keys, values, to_string, contains, starts_with, map, reverse, sum, avg). An expression always evaluates to exactly ONE JSON value — 'null' when nothing matches, which is a result and not an error. The serialized value is returned in `result` and its JSON type in `kind`; pretty=true (the default) indents it, raw=true emits strings unquoted the way `aws --output text` does. String literals use single quotes; JSON literals use backticks.",
        parameters = schema_json()
    ),
)]
impl JmespathQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jmespath-query", |a: Args| {
            let result = run_jmespath(&a.expression, &a.json, a.pretty, a.raw)
                .map_err(SkillError::InvalidArgs)?;
            let kind = json_kind(&result).to_string();
            Ok(Resp { result, kind })
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
                    "expression": { "type": "string", "description": "The JMESPath expression — the same syntax as the AWS CLI --query flag. Examples: 'people[*].name', \"people[?state == 'WA'].name\", 'people[?age > `30`]', 'sort_by(people, &age)[0]', 'people[0].{who: name, howOld: age}', 'length(items)'. Quote string literals with single quotes and JSON literals with backticks." },
                    "json":       { "type": "string", "description": "The JSON document to query." },
                    "pretty":     { "type": "boolean", "default": true, "description": "Pretty-print (indent by 2 spaces) the JSON result. Default true; set false for compact single-line JSON." },
                    "raw":        { "type": "boolean", "default": false, "description": "Emit strings unquoted, like `aws --output text` or `jq -r`: a string result loses its quotes and a top-level array prints one element per line. Non-string results are unchanged. Default false." }
                },
                "required": ["expression", "json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn pretty_defaults_to_true_when_omitted() {
        let a: Args = serde_json::from_str(r#"{"expression":"a","json":"{}"}"#).unwrap();
        assert!(
            a.pretty,
            "pretty must default to true, matching the descriptor"
        );
        assert!(!a.raw, "raw must default to false, matching the descriptor");
    }
}
