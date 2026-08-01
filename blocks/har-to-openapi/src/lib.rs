//! gizza-ai/har-to-openapi — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    har: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    openapi_version: String,
    #[serde(default = "default_true")]
    parameterize_paths: bool,
    #[serde(default = "default_true")]
    infer_types: bool,
    #[serde(default = "default_true")]
    include_examples: bool,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    drop_unsuccessful: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("har").required().multiline().describe("The HAR (HTTP Archive) capture as JSON — the file DevTools produces via Network → right-click → \"Save all as HAR\" (shape: { \"log\": { \"entries\": [ … ] } }). Contains sensitive headers/cookies, so redact first (see the har-redact tool)."))
        .param(Param::enumv("format", ["yaml", "json"]).default("yaml").describe("Output serialization: 'yaml' (default) or 'json'. Both encode the same OpenAPI document."))
        .param(Param::enumv("openapi_version", ["3.0.3", "3.1.0"]).default("3.0.3").describe("OpenAPI version to stamp in the document: '3.0.3' (default) or '3.1.0'."))
        .param(Param::boolean("parameterize_paths").default(true).describe("Collapse id-like path segments (numeric ids, UUIDs, long opaque tokens) into {param} templates, so /users/1 and /users/2 become one /users/{user} path. Default true; turn off to keep every literal URL as its own path."))
        .param(Param::boolean("infer_types").default(true).describe("Infer scalar JSON-Schema types (integer/number/boolean) for query and path parameters from their observed values. Default true; when false every parameter is typed as string."))
        .param(Param::boolean("include_examples").default(true).describe("Include a captured example value alongside each parameter and request/response body schema. Default true; turn off for a schema-only spec."))
        .param(Param::string("domain").default("").describe("Case-insensitive host substring filter: keep only requests whose host contains this text (e.g. 'api.example.com'). Blank (default) keeps every host in the capture."))
        .param(Param::string("title").default("").describe("Value for info.title in the generated spec. Blank (default) infers a title from the first captured host."))
        .param(Param::boolean("drop_unsuccessful").default(false).describe("Drop any operation that never returned a 2xx response in the capture (removes error-only and never-completed calls). Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/har-to-openapi",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer a draft OpenAPI 3.x spec (YAML or JSON) from a HAR capture — endpoints, methods, params, and request/response schemas.",
    skill(
        description = "Turn a HAR (HTTP Archive) capture into a draft OpenAPI 3.x specification, browser-local. Groups the captured requests into paths + methods, derives the servers base URL from request origins, templates id-like path segments (numeric ids, UUIDs, tokens) into {param}, collects query and path parameters, and infers JSON-Schema shapes for request bodies and per-status response bodies from the captured JSON. Options: output format (yaml|json), OpenAPI version (3.0.3|3.1.0), parameterize_paths, infer scalar param types, include captured examples, a host substring filter (domain), a custom info.title, and drop operations without a 2xx response. It does NOT redact secrets — pre-redact the HAR with the har-redact tool — or guess auth/security schemes. Returns the OpenAPI document as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "har-to-openapi", |a: Args| {
            gizza_ai_har_to_openapi_core::run(
                &a.har,
                &a.format,
                &a.openapi_version,
                a.parameterize_paths,
                a.infer_types,
                a.include_examples,
                &a.domain,
                &a.title,
                a.drop_unsuccessful,
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
          "type":"object","properties":{
            "har":{"type":"string","description":"The HAR (HTTP Archive) capture as JSON — the file DevTools produces via Network → right-click → \"Save all as HAR\" (shape: { \"log\": { \"entries\": [ … ] } }). Contains sensitive headers/cookies, so redact first (see the har-redact tool)."},
            "format":{"type":"string","enum":["yaml","json"],"default":"yaml","description":"Output serialization: 'yaml' (default) or 'json'. Both encode the same OpenAPI document."},
            "openapi_version":{"type":"string","enum":["3.0.3","3.1.0"],"default":"3.0.3","description":"OpenAPI version to stamp in the document: '3.0.3' (default) or '3.1.0'."},
            "parameterize_paths":{"type":"boolean","default":true,"description":"Collapse id-like path segments (numeric ids, UUIDs, long opaque tokens) into {param} templates, so /users/1 and /users/2 become one /users/{user} path. Default true; turn off to keep every literal URL as its own path."},
            "infer_types":{"type":"boolean","default":true,"description":"Infer scalar JSON-Schema types (integer/number/boolean) for query and path parameters from their observed values. Default true; when false every parameter is typed as string."},
            "include_examples":{"type":"boolean","default":true,"description":"Include a captured example value alongside each parameter and request/response body schema. Default true; turn off for a schema-only spec."},
            "domain":{"type":"string","default":"","description":"Case-insensitive host substring filter: keep only requests whose host contains this text (e.g. 'api.example.com'). Blank (default) keeps every host in the capture."},
            "title":{"type":"string","default":"","description":"Value for info.title in the generated spec. Blank (default) infers a title from the first captured host."},
            "drop_unsuccessful":{"type":"boolean","default":false,"description":"Drop any operation that never returned a 2xx response in the capture (removes error-only and never-completed calls). Default false."}
          },"required":["har"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
