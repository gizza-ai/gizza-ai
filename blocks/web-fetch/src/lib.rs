//! gizza-ai/web-fetch — fetches a URL via wafer-run/network.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the fetch, emit
//! the flat `ToolResp`) rather than going through `run_skill`, because web-fetch's
//! success shape is the flat `ToolResp` JSON, not the `{ "result": … }` wrapper
//! `run_skill` produces.

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const DEFAULT_MAX_BYTES: usize = 1 << 20; // 1 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[derive(Serialize)]
struct ToolResp {
    status: u16,
    url: String,
    content_type: Option<String>,
    body: String,
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// web-fetch is `Input::None` — `url` is a normal required string param (it has
/// no `ref`), so there is no `url`⊕`ref` `oneOf`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("HTTP/HTTPS URL to fetch."),
        )
        .param(
            Param::integer("max_bytes").min(1.0).describe(
                "Maximum number of bytes to return (default: 1048576). Response is truncated if larger.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Trim a body to `max_bytes`. Returns `(prefix, was_truncated)`.
fn maybe_truncate(body: &[u8], max_bytes: usize) -> (&[u8], bool) {
    if body.len() > max_bytes {
        (&body[..max_bytes], true)
    } else {
        (body, false)
    }
}

/// Extract a `Content-Type` header value (case-insensitive name match,
/// first value in the multi-value list). Returns `None` if absent.
fn extract_content_type(headers: &HashMap<String, Vec<String>>) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .and_then(|(_, vs)| vs.first().cloned())
}

#[cfg(target_arch = "wasm32")]
struct WebFetch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/web-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch a URL and return its body",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Fetch a URL and return its body as text. Optionally limit the response size.",
        parameters = schema_json()
    ),
)]
impl WebFetch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // web-fetch returns the flat ToolResp JSON directly (no `{ "result": … }`
        // wrapper), so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("web-fetch")?;
    let max_bytes = args.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

    let resp = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;

    let content_type = extract_content_type(&resp.headers);
    let (bytes, truncated) = maybe_truncate(&resp.body, max_bytes);
    let body_str = String::from_utf8_lossy(bytes).into_owned();

    let tool = ToolResp {
        status: resp.status_code,
        url: args.url,
        content_type,
        body: body_str,
        truncated,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// now emits `additionalProperties: false`, which web-fetch's authored
    /// schema already had; the descriptor renders `max_bytes`'s minimum as the
    /// JSON number `1`, same as the authored schema.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "HTTP/HTTPS URL to fetch." },
                    "max_bytes": { "type": "integer", "minimum": 1, "description": "Maximum number of bytes to return (default: 1048576). Response is truncated if larger." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn maybe_truncate_clips_when_over_limit() {
        let body = b"abcdefghij";
        let (prefix, was) = maybe_truncate(body, 4);
        assert_eq!(prefix, b"abcd");
        assert!(was);
    }

    #[test]
    fn maybe_truncate_passes_through_when_within_limit() {
        let body = b"abc";
        let (prefix, was) = maybe_truncate(body, 10);
        assert_eq!(prefix, b"abc");
        assert!(!was);
    }

    #[test]
    fn maybe_truncate_handles_exact_limit() {
        let body = b"abcd";
        let (prefix, was) = maybe_truncate(body, 4);
        assert_eq!(prefix, b"abcd");
        assert!(!was, "len == cap should not truncate");
    }

    #[test]
    fn extract_content_type_matches_case_insensitively() {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            vec!["text/html; charset=utf-8".to_string()],
        );
        assert_eq!(
            extract_content_type(&headers).as_deref(),
            Some("text/html; charset=utf-8"),
        );
    }

    #[test]
    fn extract_content_type_returns_none_when_absent() {
        let mut headers = HashMap::new();
        headers.insert("X-Other".to_string(), vec!["v".to_string()]);
        assert!(extract_content_type(&headers).is_none());
    }

    #[test]
    fn extract_content_type_picks_first_value_when_multiple() {
        let mut headers = HashMap::new();
        headers.insert(
            "content-type".to_string(),
            vec!["application/json".to_string(), "text/plain".to_string()],
        );
        assert_eq!(
            extract_content_type(&headers).as_deref(),
            Some("application/json"),
        );
    }
}
