//! gizza-ai/openapi-to-fetch-client — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const INPUT_FORMATS: [&str; 3] = ["auto", "json", "yaml"];
const STYLES: [&str; 2] = ["functions", "class"];
const NAMING: [&str; 2] = ["operation_id", "path"];
const PARAM_STYLES: [&str; 2] = ["object", "positional"];
const ERROR_HANDLING: [&str; 2] = ["throw", "result"];

fn default_true() -> bool {
    true
}
fn default_indent() -> u32 {
    2
}

#[derive(Deserialize)]
struct Args {
    spec: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    style: String,
    #[serde(default)]
    client_name: String,
    #[serde(default)]
    naming: String,
    #[serde(default)]
    param_style: String,
    #[serde(default)]
    error_handling: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    types_module: String,
    #[serde(default)]
    tags: String,
    #[serde(default = "default_true")]
    jsdoc: bool,
    #[serde(default = "default_indent")]
    indent: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("spec").required().multiline().describe("OpenAPI 3.x or Swagger 2.0 document as JSON or YAML. The tool reads the paths object and emits one typed fetch call per operation."))
        .param(Param::enumv("input_format", INPUT_FORMATS).default("auto").describe("How to parse spec: auto (try JSON then YAML), json, or yaml. Default auto."))
        .param(Param::enumv("style", STYLES).default("functions").describe("Output shape: exported functions (default) or one exported client class."))
        .param(Param::string("client_name").default("ApiClient").describe("Class name when style=class. Ignored for function output. Default ApiClient."))
        .param(Param::enumv("naming", NAMING).default("operation_id").describe("Function naming: operation_id uses operationId when present and falls back to method+path; path always derives names from method+path."))
        .param(Param::enumv("param_style", PARAM_STYLES).default("object").describe("Call signature style: object emits one typed request object per operation; positional emits path params, then body, then a query/header object."))
        .param(Param::enumv("error_handling", ERROR_HANDLING).default("throw").describe("Non-2xx handling: throw emits an ApiError class and throws; result resolves to an ApiResult<T> union with data or error."))
        .param(Param::string("base_url").default("").describe("Base URL baked into the generated client. Blank uses servers[0].url for OpenAPI 3.x or scheme/host/basePath for Swagger 2.0, then an empty string."))
        .param(Param::string("types_module").default("./types").describe("Module to import local schema types from, e.g. ./types. Blank emits placeholder type aliases so the file stays standalone."))
        .param(Param::string("tags").default("").describe("Optional comma-separated tag filter. Blank generates every operation; a non-empty filter keeps operations whose tags match."))
        .param(Param::boolean("jsdoc").default(true).describe("Include JSDoc comments from operation summaries/descriptions and parameter descriptions. Default true."))
        .param(Param::integer("indent").default(2).min(0.0).max(8.0).describe("Spaces per indentation level in the generated TypeScript (0-8). Default 2."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/openapi-to-fetch-client",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a self-contained typed TypeScript fetch client from an OpenAPI paths spec.",
    skill(
        description = "Turn an OpenAPI 3.x or Swagger 2.0 JSON/YAML document into a dependency-free TypeScript fetch client. It walks paths and operations, emits exported functions or a client class, substitutes path params, serializes query/header params, wires request bodies and response types from local schema refs, and supports throw-style ApiError or result-union error handling. It does not generate model files or fetch remote refs; point types_module at a sibling schema-types file or leave it blank for placeholder aliases.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "openapi-to-fetch-client", |a: Args| {
            gizza_ai_openapi_to_fetch_client_core::generate(
                &a.spec,
                &a.input_format,
                &a.style,
                &a.client_name,
                &a.naming,
                &a.param_style,
                &a.error_handling,
                &a.base_url,
                &a.types_module,
                &a.tags,
                a.jsdoc,
                a.indent,
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

    #[test]
    fn schema_json_exposes_all_controls_without_drift() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        let names: Vec<&str> = props.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            vec![
                "spec",
                "input_format",
                "style",
                "client_name",
                "naming",
                "param_style",
                "error_handling",
                "base_url",
                "types_module",
                "tags",
                "jsdoc",
                "indent"
            ]
        );
        assert_eq!(props["spec"]["type"], json!("string"));
        assert_eq!(
            props["input_format"]["enum"],
            json!(["auto", "json", "yaml"])
        );
        assert_eq!(props["style"]["enum"], json!(["functions", "class"]));
        assert_eq!(props["naming"]["enum"], json!(["operation_id", "path"]));
        assert_eq!(
            props["param_style"]["enum"],
            json!(["object", "positional"])
        );
        assert_eq!(props["error_handling"]["enum"], json!(["throw", "result"]));
        assert_eq!(props["jsdoc"]["default"], json!(true));
        assert_eq!(props["indent"]["minimum"], json!(0));
        assert_eq!(props["indent"]["maximum"], json!(8));
    }

    #[test]
    fn defaults_generate_a_client() {
        let a: Args = serde_json::from_str(r#"{"spec":"{\"openapi\":\"3.1.0\",\"paths\":{\"/pets\":{\"get\":{\"operationId\":\"listPets\",\"responses\":{\"200\":{\"description\":\"OK\"}}}}}}"}"#).unwrap();
        let out = gizza_ai_openapi_to_fetch_client_core::generate(
            &a.spec,
            &a.input_format,
            &a.style,
            &a.client_name,
            &a.naming,
            &a.param_style,
            &a.error_handling,
            &a.base_url,
            &a.types_module,
            &a.tags,
            a.jsdoc,
            a.indent,
        )
        .unwrap();
        assert!(out.contains("export async function listPets"), "{out}");
        assert!(out.contains("export class ApiError"), "{out}");
    }
}
