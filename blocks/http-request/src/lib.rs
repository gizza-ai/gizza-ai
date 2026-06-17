//! gizza-ai/http-request — make a full HTTP request via wafer-run/network.
//!
//! Unlike `web-fetch` (GET-only, body-as-text), this block exposes the full
//! request surface: any method, custom headers, query parameters merged into
//! the URL, and an optional request body. It returns a readable report with the
//! status line, the response headers, and the (pretty-printed-if-JSON) body.

// The #[wafer_block] macro emits wasm-only registration; the host call and the
// `Args` type are only used inside that impl. The pure request-building and
// response-formatting helpers below are compiled (and unit-tested) on the host.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{SkillError, SkillResultExt};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Internal cap on the response body we render into the report. Not a user
/// knob — it exists only so a very large body can't exhaust the per-call wasm
/// fuel budget while building the report string. Bodies over this are clipped
/// and flagged via the `truncated` field.
const MAX_BODY_BYTES: usize = 1 << 20; // 1 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    query: HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Serialize)]
struct ToolResp {
    /// The method + final URL actually requested (after query merge).
    request: String,
    /// `"<code> <reason>"`, e.g. `"200 OK"`.
    status: String,
    /// Numeric status code, for programmatic use.
    status_code: u16,
    /// Response headers, one `Name: value` per line, sorted by name.
    headers: String,
    /// The response body, pretty-printed when it parses as JSON.
    body: String,
    /// True when the body was clipped to `max_bytes` before rendering.
    truncated: bool,
}

/// The methods this tool accepts. Kept in sync with the JSON-Schema `enum` in
/// `src/lib.rs`'s `skill(...)` and in `manifest.json`.
const ALLOWED_METHODS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];

/// Validate + normalize the requested method. `None` defaults to `GET`. The
/// value is upper-cased before matching so `get`/`Get`/`GET` all work. Returns
/// a user-facing `InvalidArgs` error for anything outside `ALLOWED_METHODS`.
fn normalize_method(method: Option<&str>) -> Result<String, SkillError> {
    let raw = method.unwrap_or("GET").trim();
    if raw.is_empty() {
        return Ok("GET".to_string());
    }
    let upper = raw.to_ascii_uppercase();
    if ALLOWED_METHODS.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(SkillError::InvalidArgs(format!(
            "invalid http-request args: unsupported method {raw:?} (allowed: {})",
            ALLOWED_METHODS.join(", ")
        )))
    }
}

/// Percent-encode a query key or value per RFC 3986: keep unreserved chars
/// (`A-Z a-z 0-9 - _ . ~`) and encode everything else as `%XX`. Spaces become
/// `%20`. No `url` crate dependency — keeps the wasm payload small (mirrors the
/// inline decoder in `block-utils`).
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(hex_upper(b >> 4));
                out.push(hex_upper(b & 0x0f));
            }
        }
    }
    out
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

/// Merge `query` parameters into `base`, percent-encoding each key and value.
///
/// - Empty `query` returns `base` unchanged.
/// - Picks `?` if `base` has no query string yet, otherwise `&`.
/// - Iterates the map in sorted-key order so the result is deterministic.
fn build_url(base: &str, query: &HashMap<String, String>) -> String {
    if query.is_empty() {
        return base.to_string();
    }
    let mut pairs: Vec<(&String, &String)> = query.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let encoded: Vec<String> = pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                percent_encode_component(k),
                percent_encode_component(v)
            )
        })
        .collect();
    // Choose the joiner based on whether a query string already exists. A bare
    // trailing `?` (no params after it) counts as "already started".
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}{}", encoded.join("&"))
}

/// HTTP status reason phrases (RFC 9110 + common extras). Falls back to a
/// class-based phrase so unknown codes still render a sensible status line.
fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        418 => "I'm a teapot",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => match code {
            100..=199 => "Informational",
            200..=299 => "Success",
            300..=399 => "Redirect",
            400..=499 => "Client Error",
            500..=599 => "Server Error",
            _ => "Unknown",
        },
    }
}

/// Render `"<code> <reason>"`, e.g. `format_status(404) == "404 Not Found"`.
fn format_status(code: u16) -> String {
    format!("{code} {}", reason_phrase(code))
}

/// Render multi-valued response headers as text, one `Name: value` line per
/// value, sorted by header name (then by original value order) for a stable,
/// readable report.
fn format_headers(headers: &HashMap<String, Vec<String>>) -> String {
    let mut names: Vec<&String> = headers.keys().collect();
    names.sort();
    let mut lines = Vec::new();
    for name in names {
        for value in &headers[name] {
            lines.push(format!("{name}: {value}"));
        }
    }
    lines.join("\n")
}

/// First value of a header by case-insensitive name, or `None`.
fn header_value<'a>(headers: &'a HashMap<String, Vec<String>>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .and_then(|(_, vs)| vs.first())
        .map(|s| s.as_str())
}

/// True when a `Content-Type` looks like JSON (`application/json`,
/// `*+json`, or any `…/json` subtype). Parameters (`; charset=…`) are ignored.
fn is_json_content_type(content_type: Option<&str>) -> bool {
    let Some(ct) = content_type else {
        return false;
    };
    let main = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    main == "application/json" || main.ends_with("+json") || main.ends_with("/json")
}

/// Format the response body for the report. When `content_type` is JSON and the
/// bytes parse as JSON, pretty-print them; otherwise return the UTF-8-lossy
/// text as-is. (A JSON content-type that fails to parse falls through to raw
/// text rather than erroring — we report what the server actually sent.)
fn format_body(content_type: Option<&str>, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    if is_json_content_type(content_type) {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                return pretty;
            }
        }
    }
    text.into_owned()
}

/// Trim a body to `max_bytes`. Returns `(prefix, was_truncated)`.
fn maybe_truncate(body: &[u8], max_bytes: usize) -> (&[u8], bool) {
    if body.len() > max_bytes {
        (&body[..max_bytes], true)
    } else {
        (body, false)
    }
}

#[cfg(target_arch = "wasm32")]
struct HttpRequest;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-request",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Make a full HTTP request and return status, headers, and body",
    requires = ["wafer-run/network"],
    skill(
        description = "Make a full HTTP request to an http(s) URL: choose the method, set custom headers and query parameters, and send an optional request body. Returns the status (code + reason), the response headers, and the body (pretty-printed when it is JSON). Only public URLs are allowed — loopback and private addresses are blocked.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":     { "type": "string", "description": "Absolute http or https URL to request." },
                "method":  { "type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"], "description": "HTTP method (default GET)." },
                "headers": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Request headers as a name->value map." },
                "query":   { "type": "object", "additionalProperties": { "type": "string" }, "description": "Query parameters merged into the URL as a name->value map." },
                "body":    { "type": "string", "description": "Optional raw request body. If you set a JSON content-type header, pass JSON text here as-is." }
            },
            "required": ["url"],
            "additionalProperties": false
        }"#
    ),
)]
impl HttpRequest {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("http-request")?;
    let method = normalize_method(args.method.as_deref())?;
    let url = build_url(&args.url, &args.query);

    // The network host block (wafer-run/network) takes single-valued request
    // headers and an optional body. SSRF (loopback/private addresses) is
    // enforced host-side, so we don't re-check it here.
    let req_body: Option<&[u8]> = args.body.as_deref().map(|s| s.as_bytes());
    let resp = wafer_sdk::clients::network::do_request(&method, &url, &args.headers, req_body)?;

    let content_type = header_value(&resp.headers, "content-type");
    let (bytes, truncated) = maybe_truncate(&resp.body, MAX_BODY_BYTES);
    let body_str = format_body(content_type, bytes);

    let tool = ToolResp {
        request: format!("{method} {url}"),
        status: format_status(resp.status_code),
        status_code: resp.status_code,
        headers: format_headers(&resp.headers),
        body: body_str,
        truncated,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_method ---------------------------------------------------

    #[test]
    fn normalize_method_defaults_to_get() {
        assert_eq!(normalize_method(None).unwrap(), "GET");
        assert_eq!(normalize_method(Some("")).unwrap(), "GET");
        assert_eq!(normalize_method(Some("   ")).unwrap(), "GET");
    }

    #[test]
    fn normalize_method_uppercases_known_methods() {
        assert_eq!(normalize_method(Some("post")).unwrap(), "POST");
        assert_eq!(normalize_method(Some("Patch")).unwrap(), "PATCH");
        assert_eq!(normalize_method(Some("DELETE")).unwrap(), "DELETE");
    }

    #[test]
    fn normalize_method_rejects_unknown() {
        let err = normalize_method(Some("TRACE")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported method"), "got {msg}");
        assert!(msg.contains("TRACE"), "got {msg}");
    }

    // --- build_url (query merge) --------------------------------------------

    #[test]
    fn build_url_no_query_is_unchanged() {
        let url = build_url("https://x.test/path", &HashMap::new());
        assert_eq!(url, "https://x.test/path");
    }

    #[test]
    fn build_url_appends_query_with_question_mark() {
        let mut q = HashMap::new();
        q.insert("a".to_string(), "1".to_string());
        q.insert("b".to_string(), "2".to_string());
        // Sorted-key order makes this deterministic.
        assert_eq!(
            build_url("https://x.test/p", &q),
            "https://x.test/p?a=1&b=2"
        );
    }

    #[test]
    fn build_url_uses_ampersand_when_query_already_present() {
        let mut q = HashMap::new();
        q.insert("c".to_string(), "3".to_string());
        assert_eq!(
            build_url("https://x.test/p?existing=1", &q),
            "https://x.test/p?existing=1&c=3"
        );
    }

    #[test]
    fn build_url_percent_encodes_keys_and_values() {
        let mut q = HashMap::new();
        q.insert("na me".to_string(), "a&b=c/d".to_string());
        assert_eq!(
            build_url("https://x.test/p", &q),
            "https://x.test/p?na%20me=a%26b%3Dc%2Fd"
        );
    }

    #[test]
    fn percent_encode_keeps_unreserved() {
        assert_eq!(percent_encode_component("Aa0-_.~"), "Aa0-_.~");
        assert_eq!(percent_encode_component("a b"), "a%20b");
        assert_eq!(percent_encode_component("ä"), "%C3%A4");
    }

    // --- format_status ------------------------------------------------------

    #[test]
    fn format_status_known_codes() {
        assert_eq!(format_status(200), "200 OK");
        assert_eq!(format_status(404), "404 Not Found");
        assert_eq!(format_status(500), "500 Internal Server Error");
    }

    #[test]
    fn format_status_unknown_code_uses_class_phrase() {
        assert_eq!(format_status(299), "299 Success");
        assert_eq!(format_status(451), "451 Client Error");
        assert_eq!(format_status(599), "599 Server Error");
    }

    // --- format_headers -----------------------------------------------------

    #[test]
    fn format_headers_sorted_and_multivalue() {
        let mut h = HashMap::new();
        h.insert(
            "Set-Cookie".to_string(),
            vec!["a=1".to_string(), "b=2".to_string()],
        );
        h.insert(
            "Content-Type".to_string(),
            vec!["application/json".to_string()],
        );
        assert_eq!(
            format_headers(&h),
            "Content-Type: application/json\nSet-Cookie: a=1\nSet-Cookie: b=2"
        );
    }

    #[test]
    fn format_headers_empty() {
        assert_eq!(format_headers(&HashMap::new()), "");
    }

    // --- is_json_content_type / format_body ---------------------------------

    #[test]
    fn is_json_content_type_variants() {
        assert!(is_json_content_type(Some("application/json")));
        assert!(is_json_content_type(Some(
            "application/json; charset=utf-8"
        )));
        assert!(is_json_content_type(Some("application/vnd.api+json")));
        assert!(is_json_content_type(Some("text/json")));
        assert!(!is_json_content_type(Some("text/html")));
        assert!(!is_json_content_type(None));
    }

    #[test]
    fn format_body_pretty_prints_json() {
        let pretty = format_body(Some("application/json"), br#"{"b":2,"a":1}"#);
        // serde pretty-print is multi-line + indented.
        assert!(pretty.contains("\n  \"a\": 1"), "got {pretty:?}");
        assert!(pretty.contains("\"b\": 2"), "got {pretty:?}");
    }

    #[test]
    fn format_body_passes_through_non_json() {
        let body = format_body(Some("text/html"), b"<html>ok</html>");
        assert_eq!(body, "<html>ok</html>");
    }

    #[test]
    fn format_body_json_ct_but_invalid_json_falls_back_to_raw() {
        let body = format_body(Some("application/json"), b"not json at all");
        assert_eq!(body, "not json at all");
    }

    // --- maybe_truncate -----------------------------------------------------

    #[test]
    fn maybe_truncate_clips_over_limit() {
        let (p, was) = maybe_truncate(b"abcdef", 3);
        assert_eq!(p, b"abc");
        assert!(was);
    }

    #[test]
    fn maybe_truncate_passes_within_limit() {
        let (p, was) = maybe_truncate(b"abc", 10);
        assert_eq!(p, b"abc");
        assert!(!was);
    }
}
