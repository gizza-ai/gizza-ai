//! gizza-ai/http-request-builder core — build a well-formed raw HTTP/1.1 request
//! message from a method, URL, headers, and body. Nothing is sent. Pure-Rust
//! (`url`). Lines are CRLF-terminated per RFC 7230.

use url::Url;

/// Build the raw HTTP/1.1 request text.
/// * `method` is upper-cased (default GET when empty).
/// * `url` provides the request-target and the `Host` header.
/// * `headers` is free text, one `Name: Value` per line (blank lines ignored).
/// * `body` is appended after the blank line; a `Content-Length` is added when a
///   body is present and the user didn't supply one.
pub fn build(method: &str, url: &str, headers: &str, body: &str) -> Result<String, String> {
    let method = {
        let m = method.trim().to_ascii_uppercase();
        if m.is_empty() { "GET".to_string() } else { m }
    };
    let parsed = Url::parse(url.trim()).map_err(|e| format!("invalid URL: {e}"))?;
    if parsed.host_str().is_none() {
        return Err("URL has no host".into());
    }

    // request-target = path [?query]
    let mut target = parsed.path().to_string();
    if target.is_empty() {
        target.push('/');
    }
    if let Some(q) = parsed.query() {
        target.push('?');
        target.push_str(q);
    }

    // Host header value: host[:port] when the port is non-default.
    let host = {
        let h = parsed.host_str().unwrap().to_string();
        match parsed.port() {
            Some(p) => format!("{h}:{p}"),
            None => h,
        }
    };

    // Parse user headers (preserve order; track which names are present).
    let mut user: Vec<(String, String)> = Vec::new();
    let mut have_host = false;
    let mut have_clen = false;
    for line in headers.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let (name, value) = match l.split_once(':') {
            Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
            None => return Err(format!("malformed header line (missing ':'): {l}")),
        };
        let lname = name.to_ascii_lowercase();
        if lname == "host" {
            have_host = true;
        }
        if lname == "content-length" {
            have_clen = true;
        }
        user.push((name, value));
    }

    let mut out = String::new();
    out.push_str(&format!("{method} {target} HTTP/1.1\r\n"));
    if !have_host {
        out.push_str(&format!("Host: {host}\r\n"));
    }
    for (n, v) in &user {
        out.push_str(&format!("{n}: {v}\r\n"));
    }
    if !body.is_empty() && !have_clen {
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("\r\n");
    out.push_str(body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_get() {
        let r = build("get", "https://example.com/path?a=1", "", "").unwrap();
        assert_eq!(r, "GET /path?a=1 HTTP/1.1\r\nHost: example.com\r\n\r\n");
    }

    #[test]
    fn root_path_when_empty() {
        let r = build("GET", "https://example.com", "", "").unwrap();
        assert!(r.starts_with("GET / HTTP/1.1\r\nHost: example.com\r\n"));
    }

    #[test]
    fn post_with_body_adds_content_length() {
        let r = build("POST", "http://api.test/x", "Content-Type: application/json", "{\"a\":1}").unwrap();
        assert!(r.contains("POST /x HTTP/1.1\r\n"));
        assert!(r.contains("Host: api.test\r\n"));
        assert!(r.contains("Content-Type: application/json\r\n"));
        assert!(r.contains("Content-Length: 7\r\n"));
        assert!(r.ends_with("\r\n\r\n{\"a\":1}"));
    }

    #[test]
    fn non_default_port_in_host() {
        let r = build("GET", "http://localhost:8080/", "", "").unwrap();
        assert!(r.contains("Host: localhost:8080\r\n"));
    }

    #[test]
    fn user_host_and_content_length_respected() {
        let r = build("POST", "https://x.com/", "Host: override.com\nContent-Length: 99", "body").unwrap();
        assert!(r.contains("Host: override.com\r\n"));
        assert!(!r.contains("Host: x.com\r\n"));
        // user-provided Content-Length kept, none auto-added
        assert_eq!(r.matches("Content-Length").count(), 1);
        assert!(r.contains("Content-Length: 99\r\n"));
    }

    #[test]
    fn errors() {
        assert!(build("GET", "not a url", "", "").is_err());
        assert!(build("GET", "https://x.com/", "BadHeaderNoColon", "").is_err());
    }
}
