//! gizza-ai/openapi-to-curl — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const INPUT_FORMATS: [&str; 3] = ["auto", "json", "yaml"];
const AUTH_MODES: [&str; 5] = ["auto", "none", "bearer", "basic", "api_key"];
const OUTPUT_FORMATS: [&str; 4] = ["shell", "commands", "markdown", "json"];

fn default_true() -> bool {
    true
}
fn default_depth() -> u32 {
    4
}

#[derive(Deserialize)]
struct Args {
    spec: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    auth: String,
    #[serde(default)]
    auth_value: String,
    #[serde(default)]
    methods: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    path_filter: String,
    #[serde(default)]
    include_optional: bool,
    #[serde(default)]
    output_format: String,
    #[serde(default = "default_true")]
    multiline: bool,
    #[serde(default)]
    pretty_body: bool,
    #[serde(default = "default_true")]
    include_comments: bool,
    #[serde(default = "default_depth")]
    max_depth: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("spec").required().multiline().describe("OpenAPI 3.x or Swagger 2.0 document as JSON or YAML. Every operation under paths becomes one curl command with sample path, query, header, and body values."))
        .param(Param::enumv("input_format", INPUT_FORMATS).default("auto").describe("How to parse spec: auto (try JSON then YAML), json, or yaml. Default auto."))
        .param(Param::string("base_url").default("").describe("Base URL for the generated commands, e.g. https://api.example.com/v1. Blank uses servers[0].url (OpenAPI 3.x) or scheme/host/basePath (Swagger 2.0), then https://api.example.com as a last resort."))
        .param(Param::enumv("auth", AUTH_MODES).default("auto").describe("Credentials to add: auto follows the spec's security schemes, none omits credentials, bearer sends an Authorization: Bearer header, basic sends curl -u, api_key sends the spec's apiKey header/query/cookie (X-API-Key when the spec declares none). Default auto."))
        .param(Param::string("auth_value").default("").describe("Literal credential to embed, e.g. an actual token or user:password for basic. Blank emits shell placeholders ($TOKEN, $API_USER:$API_PASSWORD, $API_KEY) that the shell script declares up top."))
        .param(Param::string("methods").default("").describe("Optional comma-separated HTTP method filter, e.g. get,post. Blank generates every method (get, put, post, delete, options, head, patch, trace)."))
        .param(Param::string("tags").default("").describe("Optional comma-separated tag filter, e.g. pets,admin. Blank generates every operation; a non-empty filter keeps operations carrying any of those tags."))
        .param(Param::string("path_filter").default("").describe("Optional case-insensitive substring the path must contain, e.g. /pets. Blank generates every path."))
        .param(Param::boolean("include_optional").default(false).describe("Include optional query parameters, optional headers, and optional body fields as well as the required ones. Default false, which keeps each command minimal."))
        .param(Param::enumv("output_format", OUTPUT_FORMATS).default("shell").describe("shell writes a runnable bash script with BASE_URL and credential variables; commands writes bare curl lines with absolute URLs; markdown writes a heading plus fenced block per endpoint; json writes a machine-readable array of operations. Default shell."))
        .param(Param::boolean("multiline").default(true).describe("Wrap each command over several lines with trailing backslashes. Uncheck for one command per line, easier to pipe or grep. Default true."))
        .param(Param::boolean("pretty_body").default(false).describe("Pretty-print JSON request bodies across multiple lines instead of one compact line. Default false."))
        .param(Param::boolean("include_comments").default(true).describe("Add a comment line above each command with the method, path, and summary (shell and commands output only). Default true."))
        .param(Param::integer("max_depth").default(4).min(1.0).max(8.0).describe("How deep nested schemas are expanded when building sample bodies (1-8). Deeper levels collapse to null. Default 4."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/openapi-to-curl",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a ready-to-run curl command for every endpoint in an OpenAPI or Swagger spec.",
    skill(
        description = "Turn an OpenAPI 3.x or Swagger 2.0 JSON/YAML document into runnable curl examples — one per operation. It resolves the server URL, substitutes path parameters, appends query parameters, adds header and cookie parameters, builds a sample JSON/form/multipart request body from the schema (example, default, enum, and format values), and attaches bearer, basic, or api-key credentials from the spec's security schemes. Output as a bash script with variables, bare commands, markdown docs, or JSON. Filters by method, tag, or path. Nothing is sent and remote $refs are never fetched: local refs expand, external ones collapse to null.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "openapi-to-curl", |a: Args| {
            gizza_ai_openapi_to_curl_core::generate(
                &a.spec,
                &a.input_format,
                &a.base_url,
                &a.auth,
                &a.auth_value,
                &a.methods,
                &a.tags,
                &a.path_filter,
                a.include_optional,
                &a.output_format,
                a.multiline,
                a.pretty_body,
                a.include_comments,
                a.max_depth,
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
    use serde_json::json;

    const SPEC: &str = r##"{"openapi":"3.1.0","servers":[{"url":"https://api.example.com"}],"paths":{"/pets":{"get":{"operationId":"listPets","responses":{"200":{"description":"OK"}}}}}}"##;

    /// Drift guard: the schema chat/CLI/page all consume, authored literally.
    #[test]
    fn schema_json_matches_the_authored_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["spec"],
            "properties": {
                "spec": {
                    "type": "string",
                    "description": "OpenAPI 3.x or Swagger 2.0 document as JSON or YAML. Every operation under paths becomes one curl command with sample path, query, header, and body values."
                },
                "input_format": {
                    "type": "string",
                    "enum": ["auto", "json", "yaml"],
                    "default": "auto",
                    "description": "How to parse spec: auto (try JSON then YAML), json, or yaml. Default auto."
                },
                "base_url": {
                    "type": "string",
                    "default": "",
                    "description": "Base URL for the generated commands, e.g. https://api.example.com/v1. Blank uses servers[0].url (OpenAPI 3.x) or scheme/host/basePath (Swagger 2.0), then https://api.example.com as a last resort."
                },
                "auth": {
                    "type": "string",
                    "enum": ["auto", "none", "bearer", "basic", "api_key"],
                    "default": "auto",
                    "description": "Credentials to add: auto follows the spec's security schemes, none omits credentials, bearer sends an Authorization: Bearer header, basic sends curl -u, api_key sends the spec's apiKey header/query/cookie (X-API-Key when the spec declares none). Default auto."
                },
                "auth_value": {
                    "type": "string",
                    "default": "",
                    "description": "Literal credential to embed, e.g. an actual token or user:password for basic. Blank emits shell placeholders ($TOKEN, $API_USER:$API_PASSWORD, $API_KEY) that the shell script declares up top."
                },
                "methods": {
                    "type": "string",
                    "default": "",
                    "description": "Optional comma-separated HTTP method filter, e.g. get,post. Blank generates every method (get, put, post, delete, options, head, patch, trace)."
                },
                "tags": {
                    "type": "string",
                    "default": "",
                    "description": "Optional comma-separated tag filter, e.g. pets,admin. Blank generates every operation; a non-empty filter keeps operations carrying any of those tags."
                },
                "path_filter": {
                    "type": "string",
                    "default": "",
                    "description": "Optional case-insensitive substring the path must contain, e.g. /pets. Blank generates every path."
                },
                "include_optional": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include optional query parameters, optional headers, and optional body fields as well as the required ones. Default false, which keeps each command minimal."
                },
                "output_format": {
                    "type": "string",
                    "enum": ["shell", "commands", "markdown", "json"],
                    "default": "shell",
                    "description": "shell writes a runnable bash script with BASE_URL and credential variables; commands writes bare curl lines with absolute URLs; markdown writes a heading plus fenced block per endpoint; json writes a machine-readable array of operations. Default shell."
                },
                "multiline": {
                    "type": "boolean",
                    "default": true,
                    "description": "Wrap each command over several lines with trailing backslashes. Uncheck for one command per line, easier to pipe or grep. Default true."
                },
                "pretty_body": {
                    "type": "boolean",
                    "default": false,
                    "description": "Pretty-print JSON request bodies across multiple lines instead of one compact line. Default false."
                },
                "include_comments": {
                    "type": "boolean",
                    "default": true,
                    "description": "Add a comment line above each command with the method, path, and summary (shell and commands output only). Default true."
                },
                "max_depth": {
                    "type": "integer",
                    "default": 4,
                    "minimum": 1,
                    "maximum": 8,
                    "description": "How deep nested schemas are expanded when building sample bodies (1-8). Deeper levels collapse to null. Default 4."
                }
            }
        });
        assert_eq!(schema, authored);
        // Field order is load-bearing for the page form.
        let names: Vec<&str> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names,
            vec![
                "spec",
                "input_format",
                "base_url",
                "auth",
                "auth_value",
                "methods",
                "tags",
                "path_filter",
                "include_optional",
                "output_format",
                "multiline",
                "pretty_body",
                "include_comments",
                "max_depth"
            ]
        );
    }

    #[test]
    fn defaults_generate_a_shell_script() {
        let a: Args = serde_json::from_str(&format!(r#"{{"spec":{}}}"#, json!(SPEC))).unwrap();
        let out = gizza_ai_openapi_to_curl_core::generate(
            &a.spec,
            &a.input_format,
            &a.base_url,
            &a.auth,
            &a.auth_value,
            &a.methods,
            &a.tags,
            &a.path_filter,
            a.include_optional,
            &a.output_format,
            a.multiline,
            a.pretty_body,
            a.include_comments,
            a.max_depth,
        )
        .unwrap();
        assert!(out.starts_with("#!/usr/bin/env bash"), "{out}");
        assert!(out.contains("curl -X GET \\\n  \"$BASE_URL/pets\""), "{out}");
        assert_eq!(a.max_depth, 4);
        assert!(a.multiline && a.include_comments && !a.include_optional);
    }

    #[test]
    fn a_bad_spec_is_an_invalid_args_error() {
        let a: Args = serde_json::from_str(r#"{"spec":"not a spec"}"#).unwrap();
        let err = gizza_ai_openapi_to_curl_core::generate(
            &a.spec, "json", "", "auto", "", "", "", "", false, "shell", true, false, true, 4,
        )
        .unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
    }
}
