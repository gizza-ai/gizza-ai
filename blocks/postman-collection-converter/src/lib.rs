//! gizza-ai/postman-collection-converter — convert a Postman Collection v2.x
//! export or an Insomnia JSON export (format 4) into curl commands, JavaScript
//! fetch() calls, or axios calls. Chat schema single-sourced from descriptor()
//! (which also drives the CLI); handler delegates to run_skill. Pure → all
//! backends (no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    collection: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    variables: String,
    #[serde(default = "default_multiline")]
    multiline: bool,
}

fn default_multiline() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("collection")
                .required()
                .describe("The exported collection JSON, pasted verbatim: a Postman Collection v2.0/v2.1 export or an Insomnia JSON export (format 4). The format is auto-detected. Up to 200 requests per collection."),
        )
        .param(
            Param::enumv("target", ["curl", "fetch", "axios"])
                .default("curl")
                .describe("Output language: 'curl' (default) emits one curl command per request; 'fetch' emits JavaScript fetch() snippets; 'axios' emits JavaScript axios() snippets."),
        )
        .param(
            Param::string("variables")
                .default("")
                .describe("Optional values for {{placeholders}}: a Postman environment export JSON, a plain JSON object like {\"baseUrl\":\"https://api.example.com\"}, or KEY=VALUE lines (one per line). These override collection variables and Insomnia environment data; unresolved placeholders are left as-is."),
        )
        .param(
            Param::boolean("multiline")
                .default(true)
                .describe("When true (default), format each curl command across multiple lines with backslash continuations (one flag per line); when false, one line per command. Ignored for fetch and axios output."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/postman-collection-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Postman or Insomnia collection to curl, fetch, or axios code",
    skill(
        description = "Convert an API collection export into ready-to-run code. Paste a Postman Collection v2.0/v2.1 JSON export or an Insomnia JSON export (format 4) as collection (auto-detected) and pick target='curl' (default), 'fetch', or 'axios' to get one labeled snippet per request (folder path + request name as a comment). Covered per request: HTTP method, URL, headers (disabled entries skipped), body modes raw/JSON (--data-raw / JSON.stringify / axios data object), x-www-form-urlencoded (--data-urlencode / URLSearchParams), multipart form-data (-F / FormData, file fields as @path placeholders), file bodies, and GraphQL (wrapped as a JSON body), plus auth: basic (-u / base64 Authorization header / axios auth option), bearer token, and API key in a header or the query string — request-level or inherited from the collection. {{variable}} placeholders are filled from collection variables and Insomnia environments; pass variables (a Postman environment export JSON, a plain JSON object, or KEY=VALUE lines) to add or override values — unresolved placeholders stay verbatim. multiline=true (default) formats curl with backslash continuations. Limit: 200 requests per collection. Pre-request/test scripts and dynamic values like {{$guid}} are not executed. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "postman-collection-converter", |a: Args| {
            gizza_ai_postman_collection_converter_core::convert(
                &a.collection,
                &a.target,
                &a.variables,
                a.multiline,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "collection": { "type": "string", "description": "The exported collection JSON, pasted verbatim: a Postman Collection v2.0/v2.1 export or an Insomnia JSON export (format 4). The format is auto-detected. Up to 200 requests per collection." },
                    "target": { "type": "string", "enum": ["curl", "fetch", "axios"], "default": "curl", "description": "Output language: 'curl' (default) emits one curl command per request; 'fetch' emits JavaScript fetch() snippets; 'axios' emits JavaScript axios() snippets." },
                    "variables": { "type": "string", "default": "", "description": "Optional values for {{placeholders}}: a Postman environment export JSON, a plain JSON object like {\"baseUrl\":\"https://api.example.com\"}, or KEY=VALUE lines (one per line). These override collection variables and Insomnia environment data; unresolved placeholders are left as-is." },
                    "multiline": { "type": "boolean", "default": true, "description": "When true (default), format each curl command across multiple lines with backslash continuations (one flag per line); when false, one line per command. Ignored for fetch and axios output." }
                },
                "required": ["collection"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The Args struct must accept the same shapes the schema advertises.
    #[test]
    fn args_defaults_match_schema_defaults() {
        let a: Args = serde_json::from_str(r#"{"collection":"{}"}"#).unwrap();
        assert_eq!(a.target, "");
        assert_eq!(a.variables, "");
        assert!(a.multiline);
    }
}
