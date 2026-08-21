//! gizza-ai/openapi-stub-from-json — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_openapi_stub_from_json_core::{Options, METHODS, OUTPUT_FORMATS, SECURITY_KINDS};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}
fn default_status() -> u16 {
    200
}

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    request_json: String,
    #[serde(default)]
    response_json: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
    #[serde(default = "default_status")]
    status: u16,
    #[serde(default)]
    content_type: String,
    #[serde(default)]
    operation_id: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    api_version: String,
    #[serde(default)]
    server_url: String,
    #[serde(default)]
    security: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_true")]
    components: bool,
    #[serde(default)]
    extract_nested: bool,
    #[serde(default = "default_true")]
    required_props: bool,
    #[serde(default = "default_true")]
    detect_formats: bool,
    #[serde(default = "default_true")]
    include_examples: bool,
    #[serde(default)]
    include_error_responses: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("request_json").default("").multiline().describe("Sample request body as JSON, e.g. {\"name\":\"Ada\",\"age\":36}. Its inferred schema becomes the operation's requestBody. Leave blank for operations without a body (GET, DELETE); at least one of request_json / response_json must be filled in."))
        .param(Param::string("response_json").default("").multiline().describe("Sample response body as JSON, e.g. {\"id\":7,\"name\":\"Ada\"}. Its inferred schema becomes the response for the given status code. Leave blank for empty responses (204 No Content); at least one of request_json / response_json must be filled in."))
        .param(Param::enumv("method", METHODS).default("post").describe("HTTP method for the generated operation: get, post, put, patch, delete, head or options. Default post."))
        .param(Param::string("path").default("/items").describe("Request path for the operation, starting with '/'. Braced segments become path parameters — '/users/{userId}' emits a required userId parameter. Default /items."))
        .param(Param::string("query").default("").describe("Optional sample query string, e.g. 'limit=10&active=true'. Each pair becomes an optional query parameter with a type inferred from its value. Blank (default) emits no query parameters."))
        .param(Param::integer("status").default(200).min(100.0).max(599.0).describe("HTTP status code the sample response body belongs to (100-599). Its standard reason phrase becomes the response description. Default 200."))
        .param(Param::string("content_type").default("application/json").describe("Media type for the request and response bodies, e.g. application/json or application/vnd.api+json. Default application/json."))
        .param(Param::string("operation_id").default("").describe("Value for operationId, e.g. 'createUser'. Blank (default) derives one from the method and path (post /users → postUsers). Also names the generated component schemas."))
        .param(Param::string("tag").default("").describe("Operation tag used to group the endpoint in docs. Blank (default) uses the first literal path segment."))
        .param(Param::string("title").default("").describe("Value for info.title in the generated document. Blank (default) uses 'Sample API'."))
        .param(Param::string("api_version").default("").describe("Value for info.version in the generated document. Blank (default) uses '1.0.0'."))
        .param(Param::string("server_url").default("").describe("Base URL for servers[0].url, e.g. https://api.example.com/v1. Blank (default) omits the servers block entirely."))
        .param(Param::enumv("security", SECURITY_KINDS).default("none").describe("Security scheme to declare and attach to the operation: 'none' (default), 'bearer' (HTTP bearer/JWT), 'basic' (HTTP basic) or 'apiKey' (X-API-Key header)."))
        .param(Param::enumv("format", OUTPUT_FORMATS).default("yaml").describe("Output serialization: 'yaml' (default) or 'json'. Both encode the same OpenAPI 3.1 document."))
        .param(Param::boolean("components").default(true).describe("Put the request/response body schemas under components.schemas and reference them with $ref. Default true; turn off to inline each schema at its use site."))
        .param(Param::boolean("extract_nested").default(false).describe("Also hoist nested objects (and array item objects) into their own component schemas, so {\"customer\":{...}} becomes a $ref to a Customer schema. Default false. Enabling this implies component schemas."))
        .param(Param::boolean("required_props").default(true).describe("List the keys observed in the sample under 'required'. Default true. For arrays of objects only keys present in EVERY element stay required."))
        .param(Param::boolean("detect_formats").default(true).describe("Detect string formats from the sample values: date-time, date, email, uuid, uri and ipv4. Default true; turn off for plain string types."))
        .param(Param::boolean("include_examples").default(true).describe("Carry the sample bodies (and query values) through as OpenAPI 'example' values next to their schemas. Default true; turn off for a schema-only stub."))
        .param(Param::boolean("include_error_responses").default(false).describe("Add generic 400 and 500 responses backed by a shared Error schema ({error, message}). Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Options {
    let d = Options::default();
    Options {
        method: pick(&a.method, &d.method),
        path: pick(&a.path, &d.path),
        operation_id: a.operation_id.clone(),
        tag: a.tag.clone(),
        status: a.status,
        content_type: pick(&a.content_type, &d.content_type),
        title: a.title.clone(),
        api_version: a.api_version.clone(),
        server_url: a.server_url.clone(),
        query: a.query.clone(),
        security: pick(&a.security, &d.security),
        format: pick(&a.format, &d.format),
        components: a.components,
        extract_nested: a.extract_nested,
        required_props: a.required_props,
        detect_formats: a.detect_formats,
        include_examples: a.include_examples,
        include_error_responses: a.include_error_responses,
    }
}

/// Blank (an omitted CLI/chat arg) means "use the documented default".
fn pick(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/openapi-stub-from-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an OpenAPI 3.1 path-and-operation stub (with component schemas) from sample request and response JSON.",
    skill(
        description = "Turn a sample request body and/or response body into a ready-to-paste OpenAPI 3.1 path-and-operation stub, browser-local. Infers a JSON Schema for each sample (objects, arrays merged across every element, integers vs numbers, nulls, and string formats like date-time/email/uuid/uri/ipv4), then assembles paths → method → parameters/requestBody/responses with the schemas placed under components.schemas and referenced by $ref. Options: HTTP method, path (braced segments become path parameters), a sample query string (typed query parameters), status code, content type, operationId, tag, info.title/version, servers URL, a security scheme (bearer|basic|apiKey), YAML or JSON output, component vs inline schemas, nested-object extraction, required lists, format detection, example values, and generic 400/500 error responses. It generates ONE operation per run and can only describe what the sample contains — enums, validation constraints, headers and auth scopes must be added by hand. Returns the OpenAPI document as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "openapi-stub-from-json", |a: Args| {
            gizza_ai_openapi_stub_from_json_core::generate(
                &a.request_json,
                &a.response_json,
                &options_from(&a),
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
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn blank_args_fall_back_to_documented_defaults() {
        let a: Args = serde_json::from_str(r#"{"response_json":"{\"ok\":true}"}"#).unwrap();
        let o = options_from(&a);
        assert_eq!(o.method, "post");
        assert_eq!(o.path, "/items");
        assert_eq!(o.content_type, "application/json");
        assert_eq!(o.format, "yaml");
        assert_eq!(o.security, "none");
        assert_eq!(o.status, 200);
        assert!(o.components && o.required_props && o.detect_formats && o.include_examples);
        assert!(!o.extract_nested && !o.include_error_responses);
        let out = gizza_ai_openapi_stub_from_json_core::generate("", &a.response_json, &o).unwrap();
        assert!(out.contains("/items:"), "{out}");
    }

    const AUTHORED: &str = r#"{
      "type":"object","properties":{
        "request_json":{"type":"string","default":"","description":"Sample request body as JSON, e.g. {\"name\":\"Ada\",\"age\":36}. Its inferred schema becomes the operation's requestBody. Leave blank for operations without a body (GET, DELETE); at least one of request_json / response_json must be filled in."},
        "response_json":{"type":"string","default":"","description":"Sample response body as JSON, e.g. {\"id\":7,\"name\":\"Ada\"}. Its inferred schema becomes the response for the given status code. Leave blank for empty responses (204 No Content); at least one of request_json / response_json must be filled in."},
        "method":{"type":"string","enum":["get","post","put","patch","delete","head","options"],"default":"post","description":"HTTP method for the generated operation: get, post, put, patch, delete, head or options. Default post."},
        "path":{"type":"string","default":"/items","description":"Request path for the operation, starting with '/'. Braced segments become path parameters — '/users/{userId}' emits a required userId parameter. Default /items."},
        "query":{"type":"string","default":"","description":"Optional sample query string, e.g. 'limit=10&active=true'. Each pair becomes an optional query parameter with a type inferred from its value. Blank (default) emits no query parameters."},
        "status":{"type":"integer","default":200,"minimum":100,"maximum":599,"description":"HTTP status code the sample response body belongs to (100-599). Its standard reason phrase becomes the response description. Default 200."},
        "content_type":{"type":"string","default":"application/json","description":"Media type for the request and response bodies, e.g. application/json or application/vnd.api+json. Default application/json."},
        "operation_id":{"type":"string","default":"","description":"Value for operationId, e.g. 'createUser'. Blank (default) derives one from the method and path (post /users → postUsers). Also names the generated component schemas."},
        "tag":{"type":"string","default":"","description":"Operation tag used to group the endpoint in docs. Blank (default) uses the first literal path segment."},
        "title":{"type":"string","default":"","description":"Value for info.title in the generated document. Blank (default) uses 'Sample API'."},
        "api_version":{"type":"string","default":"","description":"Value for info.version in the generated document. Blank (default) uses '1.0.0'."},
        "server_url":{"type":"string","default":"","description":"Base URL for servers[0].url, e.g. https://api.example.com/v1. Blank (default) omits the servers block entirely."},
        "security":{"type":"string","enum":["none","bearer","basic","apiKey"],"default":"none","description":"Security scheme to declare and attach to the operation: 'none' (default), 'bearer' (HTTP bearer/JWT), 'basic' (HTTP basic) or 'apiKey' (X-API-Key header)."},
        "format":{"type":"string","enum":["yaml","json"],"default":"yaml","description":"Output serialization: 'yaml' (default) or 'json'. Both encode the same OpenAPI 3.1 document."},
        "components":{"type":"boolean","default":true,"description":"Put the request/response body schemas under components.schemas and reference them with $ref. Default true; turn off to inline each schema at its use site."},
        "extract_nested":{"type":"boolean","default":false,"description":"Also hoist nested objects (and array item objects) into their own component schemas, so {\"customer\":{...}} becomes a $ref to a Customer schema. Default false. Enabling this implies component schemas."},
        "required_props":{"type":"boolean","default":true,"description":"List the keys observed in the sample under 'required'. Default true. For arrays of objects only keys present in EVERY element stay required."},
        "detect_formats":{"type":"boolean","default":true,"description":"Detect string formats from the sample values: date-time, date, email, uuid, uri and ipv4. Default true; turn off for plain string types."},
        "include_examples":{"type":"boolean","default":true,"description":"Carry the sample bodies (and query values) through as OpenAPI 'example' values next to their schemas. Default true; turn off for a schema-only stub."},
        "include_error_responses":{"type":"boolean","default":false,"description":"Add generic 400 and 500 responses backed by a shared Error schema ({error, message}). Default false."}
      },"additionalProperties":false
    }"#;
}
