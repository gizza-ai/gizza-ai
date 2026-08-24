//! gizza-ai/json-assertion-runner — run JSONPath assertions against a JSON payload.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure text-in/text-out tool.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    payload: String,
    assertions: String,
    #[serde(default = "default_rules_format")]
    rules_format: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default)]
    stop_on_first_failure: bool,
}

fn default_rules_format() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "text".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema, the CLI, and the page form: a JSON payload,
/// the assertion rules, and the three knobs that change how a run is parsed and reported.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("payload").required().multiline().describe(
            "JSON payload to test. Objects, arrays, and scalar JSON values are accepted.",
        ))
        .param(Param::string("assertions").required().multiline().describe(
            "Assertion rules, one per line: `<jsonpath> <operator> [expected]` — e.g. `$.status equals ok`, `$.items[*].price lte 100`, `$.user.id matches ^u-\\d+$`, `$.items length 2`. Blank lines and `#` comments are ignored. `count` asserts how many nodes the path selected; `length` asserts the size of each matched array, object, or string. A JSON array of `{path, op, expected}` objects (optional `name`) is accepted too; use it for paths containing spaces. Maximum 200 assertions.",
        ))
        .param(Param::enumv("rules_format", ["auto", "dsl", "json"]).default("auto").describe(
            "How to parse assertions: auto detects JSON arrays vs the line DSL; dsl forces one assertion per line; json expects an array of rule objects.",
        ))
        .param(Param::enumv("output", ["text", "json"]).default("text").describe(
            "Report format: text for a human-readable pass/fail summary, or json for a machine-readable report object.",
        ))
        .param(Param::boolean("case_sensitive").default(true).describe(
            "Make string equals/contains/matches checks case-sensitive. Turn off for case-insensitive comparisons.",
        ))
        .param(Param::boolean("stop_on_first_failure").default(false).describe(
            "Stop evaluating after the first failed assertion and mark the remaining rules skipped.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-assertion-runner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run JSONPath assertions against a JSON payload",
    skill(
        description = "Run declarative JSONPath assertions against a JSON payload and return a pass/fail report. Rules can be written as a compact line DSL (`$.status equals ok`, `$.items[*].price lte 100`, `$.user.id matches ^u-\\d+$`) or as a JSON array of rule objects. Operators include exists/not_exists, equals/not_equals, type, gt/gte/lt/lte, range, contains/not_contains, matches, count, length, empty, and not_empty; `count` asserts how many nodes a path selected, `length` the size of each matched node. Value comparisons must hold for every matched node, and a path matching nothing fails. The runner reports the failing JSONPath location and actual value where possible, can stop after the first failure, supports case-sensitive or case-insensitive string checks, and can render either human-readable text or machine-readable JSON. Up to 200 assertions per run; nothing is fetched, the payload is passed in.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-assertion-runner", |a: Args| {
            gizza_ai_json_assertion_runner_core::render(
                &a.payload,
                &a.assertions,
                &a.rules_format,
                a.case_sensitive,
                a.stop_on_first_failure,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "payload":{"type":"string","description":"JSON payload to test. Objects, arrays, and scalar JSON values are accepted."},
                "assertions":{"type":"string","description":"Assertion rules, one per line: `<jsonpath> <operator> [expected]` — e.g. `$.status equals ok`, `$.items[*].price lte 100`, `$.user.id matches ^u-\\d+$`, `$.items length 2`. Blank lines and `#` comments are ignored. `count` asserts how many nodes the path selected; `length` asserts the size of each matched array, object, or string. A JSON array of `{path, op, expected}` objects (optional `name`) is accepted too; use it for paths containing spaces. Maximum 200 assertions."},
                "rules_format":{"type":"string","enum":["auto","dsl","json"],"default":"auto","description":"How to parse assertions: auto detects JSON arrays vs the line DSL; dsl forces one assertion per line; json expects an array of rule objects."},
                "output":{"type":"string","enum":["text","json"],"default":"text","description":"Report format: text for a human-readable pass/fail summary, or json for a machine-readable report object."},
                "case_sensitive":{"type":"boolean","default":true,"description":"Make string equals/contains/matches checks case-sensitive. Turn off for case-insensitive comparisons."},
                "stop_on_first_failure":{"type":"boolean","default":false,"description":"Stop evaluating after the first failed assertion and mark the remaining rules skipped."}
            },
            "required":["payload","assertions"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
