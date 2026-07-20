//! gizza-ai/har-request-extract — extract the request list from a HAR capture.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_har_request_extract_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    har: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    url_contains: String,
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_format() -> String {
    "table".into()
}
fn default_status() -> String {
    "all".into()
}
fn default_sort() -> String {
    "order".into()
}

/// Single source for the chat schema (and CLI). Extraction is forgiving:
/// entries missing fields still list with `-`/null placeholders; only
/// non-JSON / non-HAR input is an error.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("har")
                .required()
                .describe("The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\"."),
        )
        .param(
            Param::enumv("format", ["table", "csv", "json", "urls"])
                .default("table")
                .describe("Output format. 'table' = aligned text columns with a summary line. 'csv' = spreadsheet-ready rows with a header (index,method,url,status,status_text,mime_type,size_bytes,time_ms,started). 'json' = array of request objects with the same fields. 'urls' = one URL per line."),
        )
        .param(
            Param::enumv("status", ["all", "2xx", "3xx", "4xx", "5xx", "errors"])
                .default("all")
                .describe("Keep only requests in a status class: '2xx' success, '3xx' redirects, '4xx' client errors, '5xx' server errors, or 'errors' = 4xx + 5xx + failed requests recorded with status 0. Default 'all'."),
        )
        .param(
            Param::string("method")
                .default("")
                .describe("Keep only requests with this HTTP method — case-insensitive exact match, e.g. GET, POST, PUT. Empty = all methods."),
        )
        .param(
            Param::string("url_contains")
                .default("")
                .describe("Keep only requests whose URL contains this text (case-insensitive substring), e.g. /api/ or .png or example.com. Empty = no URL filter."),
        )
        .param(
            Param::enumv("sort", ["order", "slowest", "largest"])
                .default("order")
                .describe("Row order: 'order' = capture order (default), 'slowest' = longest total time first, 'largest' = biggest response first. The # column always keeps each request's original capture position."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/har-request-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the request list from a HAR capture",
    skill(
        description = "Extract the request list from a HAR (HTTP Archive) capture: every request's method, URL, status code, content type, transferred size, and total time, as an aligned text table (with a count + bytes-transferred summary), CSV, JSON, or a plain URL list. Filter by status class (2xx/3xx/4xx/5xx/errors incl. failed status-0 requests), exact HTTP method, or a URL substring; sort by capture order, slowest first, or largest first. Only the request list is extracted — headers, cookies, and bodies are never output. Forgiving about incomplete entries; runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "har-request-extract", |a: Args| {
            extract(&a.har, &a.format, &a.status, &a.method, &a.url_contains, &a.sort)
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
                    "har":          { "type": "string", "description": "The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\"." },
                    "format":       { "type": "string", "enum": ["table", "csv", "json", "urls"], "default": "table", "description": "Output format. 'table' = aligned text columns with a summary line. 'csv' = spreadsheet-ready rows with a header (index,method,url,status,status_text,mime_type,size_bytes,time_ms,started). 'json' = array of request objects with the same fields. 'urls' = one URL per line." },
                    "status":       { "type": "string", "enum": ["all", "2xx", "3xx", "4xx", "5xx", "errors"], "default": "all", "description": "Keep only requests in a status class: '2xx' success, '3xx' redirects, '4xx' client errors, '5xx' server errors, or 'errors' = 4xx + 5xx + failed requests recorded with status 0. Default 'all'." },
                    "method":       { "type": "string", "default": "", "description": "Keep only requests with this HTTP method — case-insensitive exact match, e.g. GET, POST, PUT. Empty = all methods." },
                    "url_contains": { "type": "string", "default": "", "description": "Keep only requests whose URL contains this text (case-insensitive substring), e.g. /api/ or .png or example.com. Empty = no URL filter." },
                    "sort":         { "type": "string", "enum": ["order", "slowest", "largest"], "default": "order", "description": "Row order: 'order' = capture order (default), 'slowest' = longest total time first, 'largest' = biggest response first. The # column always keeps each request's original capture position." }
                },
                "required": ["har"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
