//! gizza-ai/har-redact — replace the sensitive VALUES in a HAR capture with a
//! placeholder so it is safe to attach to a bug report while its structure
//! stays intact. Chat schema single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to run_skill. Pure → runs on all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_har_redact_core::redact_har;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    har: String,
    #[serde(default = "default_true")]
    cookies: bool,
    #[serde(default = "default_true")]
    auth_headers: bool,
    #[serde(default)]
    extra_headers: String,
    #[serde(default = "default_true")]
    query_params: bool,
    #[serde(default)]
    sensitive_params: String,
    #[serde(default = "default_bodies")]
    bodies: String,
    #[serde(default = "default_placeholder")]
    placeholder: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    pretty: bool,
}

fn default_true() -> bool {
    true
}
fn default_bodies() -> String {
    "response".into()
}
fn default_placeholder() -> String {
    "[REDACTED]".into()
}
fn default_output() -> String {
    "har".into()
}

/// Single source for the chat schema (and CLI). Substitutes sensitive VALUES
/// in place (cookies, auth/API-key headers, sensitive query-string params, and
/// — per `bodies` — request/response payloads) with a placeholder, keeping the
/// header/cookie/param NAMES, URLs, methods, status codes, timings, and sizes
/// so the capture still opens in any HAR viewer (key order preserved).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("har")
                .required()
                .describe("The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\". Max 10000 entries."),
        )
        .param(
            Param::boolean("cookies")
                .default(true)
                .describe("Redact cookie values: the request/response cookies[].value arrays and the Cookie/Set-Cookie header values. Default true."),
        )
        .param(
            Param::boolean("auth_headers")
                .default(true)
                .describe("Redact the values of Authorization, Proxy-Authorization, and common API-key/token headers (x-api-key, api-key, x-auth-token, x-csrf-token, and similar). Header names are kept. Default true."),
        )
        .param(
            Param::string("extra_headers")
                .default("")
                .describe("Comma-separated ADDITIONAL header names (case-insensitive, exact match) whose values to redact on top of the built-in auth/API-key list, e.g. x-tenant-id,x-trace. Empty (default) uses only the built-ins."),
        )
        .param(
            Param::boolean("query_params")
                .default(true)
                .describe("Redact the values of sensitive query-string parameters (token, password, code, client_secret, sig, and similar) in both request.queryString[] and the request.url query string; non-sensitive params and the URL path are kept. Default true."),
        )
        .param(
            Param::string("sensitive_params")
                .default("")
                .describe("Comma-separated ADDITIONAL query/form parameter names (case-insensitive, exact match) to treat as sensitive, merged with the built-in list, e.g. account_id,tenant. Empty (default) uses only the built-ins."),
        )
        .param(
            Param::enumv("bodies", ["none", "request", "response", "both"])
                .default("response")
                .describe("Which message bodies to replace with the placeholder: 'response' (default) response content.text, 'request' request postData (text + form params), 'both', or 'none'. Bodies often echo the redacted secrets, so the default clears response text."),
        )
        .param(
            Param::string("placeholder")
                .default("[REDACTED]")
                .describe("The text each redacted value is replaced with. Default [REDACTED]. Must not be empty; re-running with the same placeholder is a no-op (already-redacted values are left alone)."),
        )
        .param(
            Param::enumv("output", ["har", "summary"])
                .default("har")
                .describe("'har' (default) returns the redacted capture as JSON, ready to save as a .har file. 'summary' is a dry-run report instead: entries scanned and how many cookies, auth headers, query values, and body fields were redacted."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print the output HAR with 2-space indentation. Default false = compact single-line JSON (DevTools exports are pretty-printed)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/har-redact",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Redact sensitive values in a HAR capture",
    skill(
        description = "Replace the sensitive VALUES in a HAR (HTTP Archive) capture with a placeholder so it is safe to attach to a bug report, while the full structure stays intact for debugging. Redacts (each value substituted, the surrounding structure kept): cookie values (request/response cookies[] + Cookie/Set-Cookie headers), Authorization/Proxy-Authorization and common API-key/token header values, sensitive query-string parameter values (in queryString[] and the URL), and — per bodies=none/request/response/both (default response) — request postData and response content.text. Header/cookie/param names, URLs' paths, methods, status codes, timings, and sizes are untouched, so the capture still opens in any HAR viewer and the waterfall still renders (key order preserved). Extend the header list with extra_headers and the param list with sensitive_params (comma-separated). Set a custom placeholder (default [REDACTED]). Get the redacted HAR JSON (compact by default, pretty=true to indent) or a dry-run summary (output=summary) with per-category counts. Distinct from har-body-stripper, which DELETES bodies to shrink a file and leaves cookies/headers alone — this SUBSTITUTES values in place. Runs locally; max 10000 entries.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "har-redact", |a: Args| {
            redact_har(
                &a.har,
                a.cookies,
                a.auth_headers,
                &a.extra_headers,
                a.query_params,
                &a.sensitive_params,
                &a.bodies,
                &a.placeholder,
                &a.output,
                a.pretty,
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
                    "har":              { "type": "string", "description": "The HAR (HTTP Archive) capture as JSON text — the { \"log\": { \"entries\": [ … ] } } object that browser DevTools export via Network tab → \"Save all as HAR\". Max 10000 entries." },
                    "cookies":          { "type": "boolean", "default": true, "description": "Redact cookie values: the request/response cookies[].value arrays and the Cookie/Set-Cookie header values. Default true." },
                    "auth_headers":     { "type": "boolean", "default": true, "description": "Redact the values of Authorization, Proxy-Authorization, and common API-key/token headers (x-api-key, api-key, x-auth-token, x-csrf-token, and similar). Header names are kept. Default true." },
                    "extra_headers":    { "type": "string", "default": "", "description": "Comma-separated ADDITIONAL header names (case-insensitive, exact match) whose values to redact on top of the built-in auth/API-key list, e.g. x-tenant-id,x-trace. Empty (default) uses only the built-ins." },
                    "query_params":     { "type": "boolean", "default": true, "description": "Redact the values of sensitive query-string parameters (token, password, code, client_secret, sig, and similar) in both request.queryString[] and the request.url query string; non-sensitive params and the URL path are kept. Default true." },
                    "sensitive_params": { "type": "string", "default": "", "description": "Comma-separated ADDITIONAL query/form parameter names (case-insensitive, exact match) to treat as sensitive, merged with the built-in list, e.g. account_id,tenant. Empty (default) uses only the built-ins." },
                    "bodies":           { "type": "string", "enum": ["none", "request", "response", "both"], "default": "response", "description": "Which message bodies to replace with the placeholder: 'response' (default) response content.text, 'request' request postData (text + form params), 'both', or 'none'. Bodies often echo the redacted secrets, so the default clears response text." },
                    "placeholder":      { "type": "string", "default": "[REDACTED]", "description": "The text each redacted value is replaced with. Default [REDACTED]. Must not be empty; re-running with the same placeholder is a no-op (already-redacted values are left alone)." },
                    "output":           { "type": "string", "enum": ["har", "summary"], "default": "har", "description": "'har' (default) returns the redacted capture as JSON, ready to save as a .har file. 'summary' is a dry-run report instead: entries scanned and how many cookies, auth headers, query values, and body fields were redacted." },
                    "pretty":           { "type": "boolean", "default": false, "description": "Pretty-print the output HAR with 2-space indentation. Default false = compact single-line JSON (DevTools exports are pretty-printed)." }
                },
                "required": ["har"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let rendered: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(rendered, authored, "descriptor schema drifted from the authored copy");
    }
}
