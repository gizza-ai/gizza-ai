//! gizza-ai/yaml-path-query — read or edit one value in a YAML document by a
//! dotted / bracketed path. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_path_query_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    yaml: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("yaml")
                .required()
                .describe("The YAML document, pasted as text. A single document only — input holding several documents separated by '---' is rejected rather than guessed at. Anchors and aliases are resolved on read, and everything outside the edited value keeps its comments, key order, quoting style and indentation."),
        )
        .param(
            Param::string("path")
                .required()
                .describe("Where to point, in dotted / bracketed notation (lodash / dot-object style, not RFC 9535 JSONPath): 'server.host', 'items[0].name' or the equivalent 'items.0.name', and quoted keys for keys that contain a dot, a bracket or a space (['my.key'].id). A leading '$' is accepted and ignored. An empty path selects the whole document."),
        )
        .param(
            Param::enumv("mode", ["query", "set", "delete"])
                .default("query")
                .describe("What to do at the path: 'query' returns the value found there (default), 'set' writes 'value' there and returns the whole document, 'delete' removes that key or list element and returns the whole document. 'get' is accepted as an alias of 'query' and 'unset' of 'delete'."),
        )
        .param(
            Param::string("value")
                .describe("The value to write when mode is 'set'. It is parsed as YAML, so '42' becomes a number, 'true' a boolean, 'null' an empty value, and '[a, b]' or '{k: v}' inline collections; wrap it in quotes ('\"8080\"') to force a string. Ignored by 'query' and 'delete'."),
        )
        .param(
            Param::enumv("format", ["yaml", "json"])
                .default("yaml")
                .describe("Output format: 'yaml' (default) returns a scalar hit raw with no quotes and anything larger as YAML; 'json' returns pretty-printed JSON keeping the source key order. Edits requested as 'json' are re-emitted from the parsed tree, so comments survive only in 'yaml'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct YamlPathQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-path-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Query, set or delete a YAML value by a dotted path while keeping comments and formatting",
    skill(
        description = "Read or edit exactly one value inside a YAML document, addressed by a dotted / bracketed path such as `server.host`, `items[0].name`, `spec.containers[0].image` or `['key with spaces']` (lodash / dot-object notation — a leading `$` is accepted and ignored, but RFC 9535 filters and wildcards are not supported). `mode = \"query\"` (default) returns the value: a scalar comes back raw so it can be piped onward, a mapping or list comes back as YAML, and `format = \"json\"` returns pretty-printed JSON in the document's own key order. `mode = \"set\"` writes `value` at the path and returns the whole document; `value` is parsed as YAML, so `42` is a number, `true` a boolean, `null` empty, `[a, b]` and `{k: v}` inline collections, and `\"8080\"` a quoted string. Missing intermediate levels are created, and a list can be appended to by index. `mode = \"delete\"` removes the key or list element and returns the rest. Edits are made as a surgical text splice on the original source, so comments — including the trailing comment on the edited line — blank lines, key order, quoting style, indentation and flow style all survive; every splice is verified by re-parsing the result against the independently computed tree, and anything that cannot be spliced safely falls back to re-emitting the document from the tree (correct data, normalized formatting). Missing keys, out-of-range indices, descending into a scalar, malformed paths and multi-document input are reported as explicit errors instead of silently returning nothing. Fully local and deterministic — no AI model, no network, no file access.",
        parameters = schema_json()
    ),
)]
impl YamlPathQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-path-query", |a: Args| {
            run(&a.yaml, &a.path, &a.mode, &a.value, &a.format).map_err(SkillError::InvalidArgs)
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
                    "yaml": { "type": "string", "description": "The YAML document, pasted as text. A single document only — input holding several documents separated by '---' is rejected rather than guessed at. Anchors and aliases are resolved on read, and everything outside the edited value keeps its comments, key order, quoting style and indentation." },
                    "path": { "type": "string", "description": "Where to point, in dotted / bracketed notation (lodash / dot-object style, not RFC 9535 JSONPath): 'server.host', 'items[0].name' or the equivalent 'items.0.name', and quoted keys for keys that contain a dot, a bracket or a space (['my.key'].id). A leading '$' is accepted and ignored. An empty path selects the whole document." },
                    "mode": { "type": "string", "enum": ["query", "set", "delete"], "default": "query", "description": "What to do at the path: 'query' returns the value found there (default), 'set' writes 'value' there and returns the whole document, 'delete' removes that key or list element and returns the whole document. 'get' is accepted as an alias of 'query' and 'unset' of 'delete'." },
                    "value": { "type": "string", "description": "The value to write when mode is 'set'. It is parsed as YAML, so '42' becomes a number, 'true' a boolean, 'null' an empty value, and '[a, b]' or '{k: v}' inline collections; wrap it in quotes ('\"8080\"') to force a string. Ignored by 'query' and 'delete'." },
                    "format": { "type": "string", "enum": ["yaml", "json"], "default": "yaml", "description": "Output format: 'yaml' (default) returns a scalar hit raw with no quotes and anything larger as YAML; 'json' returns pretty-printed JSON keeping the source key order. Edits requested as 'json' are re-emitted from the parsed tree, so comments survive only in 'yaml'." }
                },
                "required": ["yaml", "path"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_resolve_to_a_yaml_query() {
        let a: Args =
            serde_json::from_str(r#"{"yaml":"server:\n  port: 8080\n","path":"server.port"}"#)
                .unwrap();
        assert_eq!(a.mode, "");
        assert_eq!(a.format, "");
        let out = run(&a.yaml, &a.path, &a.mode, &a.value, &a.format).unwrap();
        assert_eq!(out, "8080");
    }

    #[test]
    fn a_set_through_the_skill_path_keeps_the_comment() {
        let a: Args = serde_json::from_str(
            r##"{"yaml":"# cfg\nserver:\n  port: 8080  # listen\n","path":"server.port","mode":"set","value":"9090"}"##,
        )
        .unwrap();
        let out = run(&a.yaml, &a.path, &a.mode, &a.value, &a.format).unwrap();
        assert_eq!(out, "# cfg\nserver:\n  port: 9090  # listen\n");
    }

    #[test]
    fn an_unknown_mode_is_rejected_through_the_skill_path() {
        let err = run("a: 1\n", "a", "frobnicate", "", "yaml").unwrap_err();
        assert!(err.contains("unknown mode 'frobnicate'"), "{err}");
    }
}
