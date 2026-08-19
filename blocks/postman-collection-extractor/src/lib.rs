//! gizza-ai/postman-collection-extractor — flatten a Postman Collection export
//! into a request list (method, URL, headers, body). Chat schema single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_postman_collection_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    collection: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    url_contains: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    variables: String,
    #[serde(default = "default_true")]
    resolve_variables: bool,
}

fn default_format() -> String {
    "list".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Extraction is forgiving per
/// item (a request missing `method` lists as GET) — only non-JSON / non-
/// collection input is an error.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("collection")
                .required()
                .describe("The Postman Collection export as JSON text — a v2.0/v2.1 object with a top-level `item` array, i.e. what Postman writes for Collection → Export → Collection v2.1. Folders are flattened recursively; up to 500 requests per run."),
        )
        .param(
            Param::enumv("format", ["list", "table", "json", "csv", "markdown", "urls"])
                .default("list")
                .describe("Output format. 'list' = one block per request with folder, auth, headers and body. 'table' = aligned overview columns (#, METHOD, HDRS, BODY, NAME, URL). 'json' = array of request objects (index, folder, name, method, url, auth, headers, body_mode, body). 'csv' = spreadsheet rows with the same fields. 'markdown' = a GFM table for docs. 'urls' = one URL per line, duplicates removed."),
        )
        .param(
            Param::string("method")
                .default("")
                .describe("Keep only requests with this HTTP method — case-insensitive exact match, e.g. GET, POST, DELETE. Empty = all methods."),
        )
        .param(
            Param::string("url_contains")
                .default("")
                .describe("Keep only requests whose URL contains this text (case-insensitive substring), e.g. /v1/ or users. Empty = no URL filter."),
        )
        .param(
            Param::string("folder")
                .default("")
                .describe("Keep only requests whose folder path contains this text (case-insensitive substring). Nested folders are joined with ' / ', e.g. 'Admin / Users'. Empty = every folder plus root-level requests."),
        )
        .param(
            Param::string("variables")
                .default("")
                .describe("Extra values for {{placeholders}}, overriding the collection's own variables. Accepts KEY=VALUE lines (baseUrl=https://api.example.com), a JSON object, or a pasted Postman environment export. Empty = use only the collection's variables."),
        )
        .param(
            Param::boolean("resolve_variables")
                .default(true)
                .describe("Substitute {{placeholders}} in URLs, headers and bodies from the collection's variables plus `variables` (default true). Set false to see the raw {{placeholders}} exactly as exported. Placeholders with no value are always left verbatim."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/postman-collection-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten a Postman collection into a request list with method, URL, headers and body",
    skill(
        description = "Flatten a Postman Collection v2.0/v2.1 export into a list of its requests: folder path, name, method, URL, headers, request body, and auth type. Nested folders are walked recursively; {{variables}} are resolved from the collection's own variables plus optional KEY=VALUE / environment-export overrides (unknown placeholders are left verbatim). Output as an indented list, an aligned table, JSON, CSV, a Markdown table, or a plain deduplicated URL list; filter by HTTP method, URL substring, or folder path. Disabled headers and disabled form fields are skipped; scripts, tests and saved example responses are not extracted, and auth is reported by type only — tokens are never printed. Up to 500 requests, each body truncated at 2000 characters. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "postman-collection-extractor", |a: Args| {
            extract(
                &a.collection,
                &a.format,
                &a.method,
                &a.url_contains,
                &a.folder,
                &a.variables,
                a.resolve_variables,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "collection":        { "type": "string", "description": "The Postman Collection export as JSON text — a v2.0/v2.1 object with a top-level `item` array, i.e. what Postman writes for Collection → Export → Collection v2.1. Folders are flattened recursively; up to 500 requests per run." },
                    "format":            { "type": "string", "enum": ["list", "table", "json", "csv", "markdown", "urls"], "default": "list", "description": "Output format. 'list' = one block per request with folder, auth, headers and body. 'table' = aligned overview columns (#, METHOD, HDRS, BODY, NAME, URL). 'json' = array of request objects (index, folder, name, method, url, auth, headers, body_mode, body). 'csv' = spreadsheet rows with the same fields. 'markdown' = a GFM table for docs. 'urls' = one URL per line, duplicates removed." },
                    "method":            { "type": "string", "default": "", "description": "Keep only requests with this HTTP method — case-insensitive exact match, e.g. GET, POST, DELETE. Empty = all methods." },
                    "url_contains":      { "type": "string", "default": "", "description": "Keep only requests whose URL contains this text (case-insensitive substring), e.g. /v1/ or users. Empty = no URL filter." },
                    "folder":            { "type": "string", "default": "", "description": "Keep only requests whose folder path contains this text (case-insensitive substring). Nested folders are joined with ' / ', e.g. 'Admin / Users'. Empty = every folder plus root-level requests." },
                    "variables":         { "type": "string", "default": "", "description": "Extra values for {{placeholders}}, overriding the collection's own variables. Accepts KEY=VALUE lines (baseUrl=https://api.example.com), a JSON object, or a pasted Postman environment export. Empty = use only the collection's variables." },
                    "resolve_variables": { "type": "boolean", "default": true, "description": "Substitute {{placeholders}} in URLs, headers and bodies from the collection's variables plus `variables` (default true). Set false to see the raw {{placeholders}} exactly as exported. Placeholders with no value are always left verbatim." }
                },
                "required": ["collection"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
