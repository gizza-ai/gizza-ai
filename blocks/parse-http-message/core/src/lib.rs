//! gizza-ai/parse-http-message core — parse a raw HTTP/1.x request or response
//! message into a structured form: the message kind (request/response), the
//! start-line fields (method/target or status/reason), the HTTP version, the
//! headers (preserving order and duplicates), and the body.
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    /// "request" or "response".
    pub kind: String,
    /// HTTP version, e.g. "HTTP/1.1".
    pub http_version: String,

    // --- request-only fields ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// The request target (path + query), e.g. "/index.html?q=1".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The path portion of the request target (before any '?').
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The raw query string (after '?'), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    // --- response-only fields ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    /// The status class, e.g. "Success" for 2xx.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_class: Option<String>,

    /// Headers in wire order, preserving duplicates.
    pub headers: Vec<Header>,
    /// Number of header lines.
    pub header_count: usize,
    /// The Content-Type header value, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The Content-Length header parsed as an integer, if present and valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_length: Option<u64>,
    /// True when a `Transfer-Encoding: chunked` header is present.
    pub chunked: bool,

    /// The message body as text (best-effort UTF-8).
    pub body: String,
    /// Body length in bytes.
    pub body_length: usize,
}

fn status_class(code: u16) -> &'static str {
    match code {
        100..=199 => "Informational",
        200..=299 => "Success",
        300..=399 => "Redirection",
        400..=499 => "Client Error",
        500..=599 => "Server Error",
        _ => "Unknown",
    }
}

/// The default reason phrase for a status code, used when the message omits one.
fn default_reason(code: u16) -> Option<&'static str> {
    Some(match code {
        100 => "Continue",
        101 => "Switching Protocols",
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
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        418 => "I'm a teapot",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => return None,
    })
}

/// Split a raw message into (head, body). The head/body boundary is the first
/// blank line (CRLF CRLF or LF LF). Returns the head and the body separately.
fn split_head_body(input: &str) -> (&str, &str) {
    // Prefer the RFC CRLFCRLF boundary, but tolerate bare LF (editors/pastes).
    if let Some(idx) = input.find("\r\n\r\n") {
        return (&input[..idx], &input[idx + 4..]);
    }
    if let Some(idx) = input.find("\n\n") {
        return (&input[..idx], &input[idx + 2..]);
    }
    (input, "")
}

/// Parse a raw HTTP/1.x request or response message.
pub fn parse(input: &str) -> Result<Message, String> {
    // Strip a leading BOM and any leading blank lines (lenient on pastes).
    let trimmed = input.trim_start_matches('\u{feff}');
    let trimmed = trimmed.trim_start_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return Err("input is empty — paste a raw HTTP request or response".into());
    }

    let (head, body) = split_head_body(trimmed);
    let mut lines = head.split('\n');

    let start_line = lines
        .next()
        .ok_or("could not read the start line")?
        .trim_end_matches('\r')
        .trim();
    if start_line.is_empty() {
        return Err("the start line is empty".into());
    }

    // Decide request vs response by the first token: a response starts with the
    // HTTP version ("HTTP/x.y"), a request ends with it.
    let parts: Vec<&str> = start_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err(format!(
            "the start line '{start_line}' is not a valid HTTP request or status line"
        ));
    }

    let is_response = parts[0].to_ascii_uppercase().starts_with("HTTP/");

    let mut msg = Message {
        kind: String::new(),
        http_version: String::new(),
        method: None,
        target: None,
        path: None,
        query: None,
        status_code: None,
        status_text: None,
        status_class: None,
        headers: Vec::new(),
        header_count: 0,
        content_type: None,
        content_length: None,
        chunked: false,
        body: String::new(),
        body_length: 0,
    };

    if is_response {
        // Status line:  HTTP/1.1 200 OK
        msg.kind = "response".into();
        msg.http_version = parts[0].to_string();
        let code: u16 = parts[1]
            .parse()
            .map_err(|_| format!("'{}' is not a valid status code", parts[1]))?;
        if !(100..=599).contains(&code) {
            return Err(format!("status code {code} is out of the 100–599 range"));
        }
        msg.status_code = Some(code);
        let reason = parts
            .get(2)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| default_reason(code).map(|s| s.to_string()))
            .unwrap_or_default();
        msg.status_text = Some(reason);
        msg.status_class = Some(status_class(code).to_string());
    } else {
        // Request line:  GET /path?q=1 HTTP/1.1
        if parts.len() < 3 {
            return Err(format!(
                "the request line '{start_line}' must be 'METHOD TARGET HTTP/x.y'"
            ));
        }
        msg.kind = "request".into();
        msg.method = Some(parts[0].to_string());
        let target = parts[1].to_string();
        let version = parts[2].trim().to_string();
        if !version.to_ascii_uppercase().starts_with("HTTP/") {
            return Err(format!(
                "'{version}' is not an HTTP version (expected HTTP/x.y) in the request line"
            ));
        }
        msg.http_version = version;
        if let Some((p, q)) = target.split_once('?') {
            msg.path = Some(p.to_string());
            msg.query = Some(q.to_string());
        } else {
            msg.path = Some(target.clone());
        }
        msg.target = Some(target);
    }

    // Headers: every remaining head line is "Name: value". A line beginning with
    // whitespace is an (obsolete) folded continuation of the previous value.
    for raw in lines {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            // obsolete line folding — append to the previous header value.
            if let Some(last) = msg.headers.last_mut() {
                last.value.push(' ');
                last.value.push_str(line.trim());
                continue;
            }
            return Err("header continuation line has no preceding header".into());
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("header line '{line}' has no ':' separator"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("header line '{line}' has an empty field name"));
        }
        let value = value.trim();
        msg.headers.push(Header {
            name: name.to_string(),
            value: value.to_string(),
        });
    }
    msg.header_count = msg.headers.len();

    // Convenience extractions (case-insensitive header lookups).
    for h in &msg.headers {
        let lname = h.name.to_ascii_lowercase();
        match lname.as_str() {
            "content-type" => {
                if msg.content_type.is_none() {
                    msg.content_type = Some(h.value.clone());
                }
            }
            "content-length" => {
                if msg.content_length.is_none() {
                    if let Ok(n) = h.value.trim().parse::<u64>() {
                        msg.content_length = Some(n);
                    }
                }
            }
            "transfer-encoding" => {
                if h.value.to_ascii_lowercase().contains("chunked") {
                    msg.chunked = true;
                }
            }
            _ => {}
        }
    }

    msg.body = body.to_string();
    msg.body_length = body.len();

    Ok(msg)
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let msg = parse(input)?;
    serde_json::to_string_pretty(&msg).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let m = parse(input)?;
    let mut out = String::new();
    if m.kind == "request" {
        out.push_str("Type:           HTTP request\n");
        out.push_str(&format!(
            "Method:         {}\n",
            m.method.as_deref().unwrap_or("")
        ));
        out.push_str(&format!(
            "Target:         {}\n",
            m.target.as_deref().unwrap_or("")
        ));
        if let Some(p) = &m.path {
            out.push_str(&format!("Path:           {p}\n"));
        }
        if let Some(q) = &m.query {
            out.push_str(&format!("Query:          {q}\n"));
        }
        out.push_str(&format!("HTTP version:   {}\n", m.http_version));
    } else {
        out.push_str("Type:           HTTP response\n");
        out.push_str(&format!("HTTP version:   {}\n", m.http_version));
        if let Some(c) = m.status_code {
            out.push_str(&format!(
                "Status:         {} {}\n",
                c,
                m.status_text.as_deref().unwrap_or("")
            ));
        }
        if let Some(cl) = &m.status_class {
            out.push_str(&format!("Status class:   {cl}\n"));
        }
    }
    out.push_str(&format!("\nHeaders ({}):\n", m.header_count));
    for h in &m.headers {
        out.push_str(&format!("  {}: {}\n", h.name, h.value));
    }
    if let Some(ct) = &m.content_type {
        out.push_str(&format!("\nContent-Type:   {ct}\n"));
    }
    if let Some(cl) = m.content_length {
        out.push_str(&format!("Content-Length: {cl}\n"));
    }
    if m.chunked {
        out.push_str("Transfer-Encoding: chunked\n");
    }
    out.push_str(&format!("\nBody ({} bytes):\n", m.body_length));
    if m.body.is_empty() {
        out.push_str("  (empty)\n");
    } else {
        out.push_str(&m.body);
        if !m.body.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &str = "GET /index.html?q=1&lang=en HTTP/1.1\r\nHost: example.com\r\nUser-Agent: curl/8.0\r\nAccept: */*\r\n\r\n";

    const POST: &str = "POST /api/login HTTP/1.1\r\nHost: api.example.com\r\nContent-Type: application/json\r\nContent-Length: 27\r\n\r\n{\"user\":\"a\",\"pass\":\"secret\"}";

    const RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 13\r\n\r\n<h1>Hello</h1>";

    #[test]
    fn parses_get_request() {
        let m = parse(REQUEST).unwrap();
        assert_eq!(m.kind, "request");
        assert_eq!(m.method.as_deref(), Some("GET"));
        assert_eq!(m.target.as_deref(), Some("/index.html?q=1&lang=en"));
        assert_eq!(m.path.as_deref(), Some("/index.html"));
        assert_eq!(m.query.as_deref(), Some("q=1&lang=en"));
        assert_eq!(m.http_version, "HTTP/1.1");
        assert_eq!(m.header_count, 3);
        assert_eq!(m.headers[0].name, "Host");
        assert_eq!(m.headers[0].value, "example.com");
        assert!(m.body.is_empty());
    }

    #[test]
    fn parses_post_with_body() {
        let m = parse(POST).unwrap();
        assert_eq!(m.method.as_deref(), Some("POST"));
        assert_eq!(m.content_type.as_deref(), Some("application/json"));
        assert_eq!(m.content_length, Some(27));
        assert_eq!(m.body, "{\"user\":\"a\",\"pass\":\"secret\"}");
        assert_eq!(m.body_length, 28);
    }

    #[test]
    fn parses_response() {
        let m = parse(RESPONSE).unwrap();
        assert_eq!(m.kind, "response");
        assert_eq!(m.status_code, Some(200));
        assert_eq!(m.status_text.as_deref(), Some("OK"));
        assert_eq!(m.status_class.as_deref(), Some("Success"));
        assert_eq!(m.http_version, "HTTP/1.1");
        assert_eq!(m.content_type.as_deref(), Some("text/html; charset=utf-8"));
        assert_eq!(m.body, "<h1>Hello</h1>");
    }

    #[test]
    fn response_default_reason_when_omitted() {
        let m = parse("HTTP/1.1 404\r\n\r\n").unwrap();
        assert_eq!(m.status_code, Some(404));
        assert_eq!(m.status_text.as_deref(), Some("Not Found"));
        assert_eq!(m.status_class.as_deref(), Some("Client Error"));
    }

    #[test]
    fn detects_chunked() {
        let m = parse("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
        assert!(m.chunked);
    }

    #[test]
    fn preserves_duplicate_headers() {
        let m = parse("HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n\r\n").unwrap();
        assert_eq!(m.header_count, 2);
        assert_eq!(m.headers[0].value, "a=1");
        assert_eq!(m.headers[1].value, "b=2");
    }

    #[test]
    fn tolerates_bare_lf() {
        let m = parse("GET / HTTP/1.1\nHost: x\n\nbody").unwrap();
        assert_eq!(m.method.as_deref(), Some("GET"));
        assert_eq!(m.headers[0].name, "Host");
        assert_eq!(m.body, "body");
    }

    #[test]
    fn folds_continuation_lines() {
        let m = parse("HTTP/1.1 200 OK\r\nX-Long: part1\r\n  part2\r\n\r\n").unwrap();
        assert_eq!(m.headers[0].value, "part1 part2");
    }

    #[test]
    fn run_emits_json() {
        let j = run(RESPONSE).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["kind"], "response");
        assert_eq!(v["status_code"], 200);
        assert_eq!(v["headers"][0]["name"], "Content-Type");
    }

    #[test]
    fn render_includes_fields() {
        let r = render(REQUEST).unwrap();
        assert!(r.contains("HTTP request"));
        assert!(r.contains("Method:"));
        assert!(r.contains("GET"));
        assert!(r.contains("Host: example.com"));
    }

    #[test]
    fn errors_on_empty() {
        assert!(parse("").is_err());
        assert!(parse("   \r\n\r\n").is_err());
    }

    #[test]
    fn errors_on_bad_status_code() {
        assert!(parse("HTTP/1.1 abc OK\r\n\r\n").is_err());
        assert!(parse("HTTP/1.1 999 X\r\n\r\n").is_err());
    }

    #[test]
    fn errors_on_bad_request_line() {
        assert!(parse("GET /path\r\n\r\n").is_err());
    }

    #[test]
    fn errors_on_header_without_colon() {
        assert!(parse("GET / HTTP/1.1\r\nBadHeader\r\n\r\n").is_err());
    }
}
