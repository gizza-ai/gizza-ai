//! gizza-ai/parse-uri core — split a URI/URL into its RFC 3986 components:
//! scheme, authority (userinfo → username/password, host, port), path, query
//! (parsed into key/value pairs), and fragment, with percent-decoded views.
//! Pure-Rust, no wafer/wasm-bindgen deps. Shared by the chat block and the page.

use serde::Serialize;

/// A single decoded `key=value` pair from the query string. `value` is `None`
/// for a bare key with no `=` (e.g. `?flag`); an empty value (`?k=`) is
/// `Some("")`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryParam {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The structured result of parsing a URI reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedUri {
    /// The scheme, lowercased (e.g. `https`), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// The raw authority component (`userinfo@host:port`), if an `//` authority
    /// is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// The raw userinfo (`user:pass`) before the `@`, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo: Option<String>,
    /// The username portion of the userinfo (percent-decoded), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// The password portion of the userinfo (percent-decoded), if a `:` is in
    /// the userinfo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// The host (registered name, IPv4, or `[IPv6]`), lowercased for reg-names,
    /// if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The port as parsed from the authority, if present and numeric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// The origin (`scheme://authority`), if both a scheme and authority are
    /// present — the security origin used for same-origin checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// The path component (may be empty).
    pub path: String,
    /// The path percent-decoded for display, if it differs from `path`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_decoded: Option<String>,
    /// The path split into its non-empty, percent-decoded segments (between
    /// `/`). Empty for a root or empty path.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path_segments: Vec<String>,
    /// The last path segment (percent-decoded), i.e. the filename, if the path
    /// has one and does not end in `/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// The extension of `filename` (without the dot), if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_extension: Option<String>,
    /// The raw query string (after `?`, before `#`), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The query parsed into decoded key/value pairs (order preserved,
    /// duplicates kept). Empty when there is no query.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub query_params: Vec<QueryParam>,
    /// The fragment (after `#`), percent-decoded, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
    /// `true` when the reference is absolute (has a scheme).
    pub is_absolute: bool,
}

/// Percent-decode a string. Invalid escapes are left literal (lenient — a URI
/// pasted with stray `%` shouldn't error). Bytes are collected and decoded as
/// UTF-8 lossily so non-UTF-8 sequences degrade to U+FFFD rather than failing.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = hex_val(bytes[i + 1]);
            let l = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Decode `application/x-www-form-urlencoded`-style query pairs: split on `&`
/// (and `;`), then `=`; a `+` is treated as a space before percent-decoding.
fn parse_query(query: &str) -> Vec<QueryParam> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split(['&', ';'])
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => QueryParam {
                key: percent_decode(&k.replace('+', " ")),
                value: Some(percent_decode(&v.replace('+', " "))),
            },
            None => QueryParam {
                key: percent_decode(&pair.replace('+', " ")),
                value: None,
            },
        })
        .collect()
}

/// Split off the userinfo / host / port from a raw authority component.
/// Returns `(userinfo, host, port)`.
fn parse_authority(authority: &str) -> (Option<String>, Option<String>, Option<u32>) {
    // userinfo is everything before the LAST '@' (a host can't contain '@', but
    // userinfo can contain a percent-encoded one — split on the last @ to be safe).
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, hp)) => (Some(u.to_string()), hp),
        None => (None, authority),
    };

    // Host may be a bracketed IPv6 literal `[...]`, optionally followed by `:port`.
    let (host, port_str) = if let Some(rest) = hostport.strip_prefix('[') {
        match rest.split_once(']') {
            Some((h6, after)) => {
                let host = format!("[{h6}]");
                let port = after.strip_prefix(':').filter(|p| !p.is_empty());
                (Some(host), port.map(str::to_string))
            }
            // Unterminated bracket — treat the whole thing as the host.
            None => (Some(hostport.to_string()), None),
        }
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (Some(h.to_string()), Some(p.to_string())),
            None => (Some(hostport.to_string()), None),
        }
    };

    // A registered name is case-insensitive — lowercase it (lowercasing digits/
    // dots/brackets in IPv4/IPv6 literals is a harmless no-op).
    let host = host
        .map(|h| h.to_ascii_lowercase())
        .filter(|h| !h.is_empty());

    let port = port_str.and_then(|p| p.parse::<u32>().ok());

    (userinfo, host, port)
}

/// Parse a URI reference into its components per RFC 3986 §3. Always succeeds
/// for non-empty input (a bare path like `foo/bar` is a valid relative
/// reference); returns `Err` only for empty/whitespace-only input.
pub fn parse(input: &str) -> Result<ParsedUri, String> {
    let uri = input.trim();
    if uri.is_empty() {
        return Err("empty input: provide a URI or URL to parse".into());
    }

    // 1. Split off the fragment (first '#').
    let (before_frag, fragment) = match uri.split_once('#') {
        Some((b, f)) => (b, Some(f.to_string())),
        None => (uri, None),
    };

    // 2. Split off the query (first '?').
    let (before_query, query) = match before_frag.split_once('?') {
        Some((b, q)) => (b, Some(q.to_string())),
        None => (before_frag, None),
    };

    // 3. Scheme: `scheme:` where scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ).
    let (scheme, after_scheme) = match split_scheme(before_query) {
        Some((s, rest)) => (Some(s.to_ascii_lowercase()), rest),
        None => (None, before_query),
    };

    // 4. Authority: present iff the part after the scheme starts with `//`.
    let (authority_raw, path) = if let Some(rest) = after_scheme.strip_prefix("//") {
        // Authority ends at the first '/' (query/fragment already stripped).
        match rest.find('/') {
            Some(idx) => (Some(rest[..idx].to_string()), rest[idx..].to_string()),
            None => (Some(rest.to_string()), String::new()),
        }
    } else {
        (None, after_scheme.to_string())
    };

    let (userinfo, host, port) = match &authority_raw {
        Some(a) => parse_authority(a),
        None => (None, None, None),
    };

    let (username, password) = match &userinfo {
        Some(ui) => match ui.split_once(':') {
            Some((u, p)) => (Some(percent_decode(u)), Some(percent_decode(p))),
            None => (Some(percent_decode(ui)), None),
        },
        None => (None, None),
    };

    let path_decoded = {
        let dec = percent_decode(&path);
        if dec != path {
            Some(dec)
        } else {
            None
        }
    };

    // Path segments: split on '/', drop the empties (leading/trailing/double),
    // percent-decode each.
    let path_segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .collect();

    // Filename = last segment, but only when the path does NOT end in '/'
    // (a trailing slash means a directory, not a file).
    let filename = if path.ends_with('/') {
        None
    } else {
        path_segments.last().cloned()
    };
    let file_extension = filename.as_deref().and_then(|f| {
        f.rsplit_once('.')
            .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty())
            .map(|(_, ext)| ext.to_string())
    });

    // Origin = scheme://host[:port], only when both a scheme and a host are
    // present. Per the origin concept, userinfo is dropped and the host is the
    // normalized (lowercased) one.
    let origin = match (&scheme, &host) {
        (Some(s), Some(h)) => Some(match port {
            Some(p) => format!("{s}://{h}:{p}"),
            None => format!("{s}://{h}"),
        }),
        _ => None,
    };

    let query_params = query.as_deref().map(parse_query).unwrap_or_default();
    let fragment = fragment.map(|f| percent_decode(&f));

    Ok(ParsedUri {
        is_absolute: scheme.is_some(),
        scheme,
        authority: authority_raw,
        userinfo,
        username,
        password,
        host,
        port,
        origin,
        path,
        path_decoded,
        path_segments,
        filename,
        file_extension,
        query,
        query_params,
        fragment,
    })
}

/// If `s` begins with a valid RFC 3986 scheme followed by `:`, return
/// `(scheme, rest_after_colon)`. The scheme must start with a letter and
/// contain only `ALPHA / DIGIT / + / - / .`.
fn split_scheme(s: &str) -> Option<(String, &str)> {
    let colon = s.find(':')?;
    let scheme = &s[..colon];
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return None,
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    Some((scheme.to_string(), &s[colon + 1..]))
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let parsed = parse(input)?;
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let p = parse(input)?;
    let mut out = String::new();
    let row = |out: &mut String, label: &str, val: &str| {
        out.push_str(&format!("{label:<12}{val}\n"));
    };
    row(
        &mut out,
        "Type:",
        if p.is_absolute {
            "absolute URI"
        } else {
            "relative reference"
        },
    );
    if let Some(s) = &p.scheme {
        row(&mut out, "Scheme:", s);
    }
    if let Some(a) = &p.authority {
        row(&mut out, "Authority:", a);
    }
    if let Some(u) = &p.username {
        row(&mut out, "Username:", u);
    }
    if let Some(pw) = &p.password {
        row(&mut out, "Password:", pw);
    }
    if let Some(h) = &p.host {
        row(&mut out, "Host:", h);
    }
    if let Some(port) = p.port {
        row(&mut out, "Port:", &port.to_string());
    }
    if let Some(o) = &p.origin {
        row(&mut out, "Origin:", o);
    }
    let path_disp = p.path_decoded.as_deref().unwrap_or(&p.path);
    row(
        &mut out,
        "Path:",
        if path_disp.is_empty() {
            "(empty)"
        } else {
            path_disp
        },
    );
    if !p.path_segments.is_empty() {
        row(&mut out, "Segments:", &p.path_segments.join("  /  "));
    }
    if let Some(f) = &p.filename {
        let line = match &p.file_extension {
            Some(ext) => format!("{f}  (.{ext})"),
            None => f.clone(),
        };
        row(&mut out, "Filename:", &line);
    }
    if let Some(q) = &p.query {
        row(&mut out, "Query:", q);
    }
    if !p.query_params.is_empty() {
        out.push_str(&format!("\nQuery params ({}):\n", p.query_params.len()));
        for qp in &p.query_params {
            match &qp.value {
                Some(v) => out.push_str(&format!("  {} = {}\n", qp.key, v)),
                None => out.push_str(&format!("  {} (no value)\n", qp.key)),
            }
        }
    }
    if let Some(f) = &p.fragment {
        out.push_str(&format!("\nFragment:   {f}\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_https_url() {
        let p = parse("https://user:pass@www.example.com:8443/a/b?x=1&y=two#sec").unwrap();
        assert_eq!(p.scheme.as_deref(), Some("https"));
        assert_eq!(p.authority.as_deref(), Some("user:pass@www.example.com:8443"));
        assert_eq!(p.userinfo.as_deref(), Some("user:pass"));
        assert_eq!(p.username.as_deref(), Some("user"));
        assert_eq!(p.password.as_deref(), Some("pass"));
        assert_eq!(p.host.as_deref(), Some("www.example.com"));
        assert_eq!(p.port, Some(8443));
        assert_eq!(p.path, "/a/b");
        assert_eq!(p.query.as_deref(), Some("x=1&y=two"));
        assert_eq!(p.query_params.len(), 2);
        assert_eq!(p.query_params[0].key, "x");
        assert_eq!(p.query_params[0].value.as_deref(), Some("1"));
        assert_eq!(p.query_params[1].key, "y");
        assert_eq!(p.query_params[1].value.as_deref(), Some("two"));
        assert_eq!(p.fragment.as_deref(), Some("sec"));
        assert!(p.is_absolute);
    }

    #[test]
    fn lowercases_scheme_and_host() {
        let p = parse("HTTPS://WWW.Example.COM/Path").unwrap();
        assert_eq!(p.scheme.as_deref(), Some("https"));
        assert_eq!(p.host.as_deref(), Some("www.example.com"));
        // Path case is preserved.
        assert_eq!(p.path, "/Path");
    }

    #[test]
    fn percent_decodes_query_and_path() {
        let p = parse("https://ex.com/a%20b/c?q=S%C3%A3o%20Paulo&plus=a+b").unwrap();
        assert_eq!(p.path, "/a%20b/c");
        assert_eq!(p.path_decoded.as_deref(), Some("/a b/c"));
        assert_eq!(p.query_params[0].value.as_deref(), Some("São Paulo"));
        // '+' in a query value decodes to a space.
        assert_eq!(p.query_params[1].value.as_deref(), Some("a b"));
    }

    #[test]
    fn ipv6_host_with_port() {
        let p = parse("http://[2001:db8::1]:8080/x").unwrap();
        assert_eq!(p.host.as_deref(), Some("[2001:db8::1]"));
        assert_eq!(p.port, Some(8080));
        assert_eq!(p.path, "/x");
    }

    #[test]
    fn ipv6_host_no_port() {
        let p = parse("http://[::1]/").unwrap();
        assert_eq!(p.host.as_deref(), Some("[::1]"));
        assert_eq!(p.port, None);
    }

    #[test]
    fn mailto_has_no_authority() {
        let p = parse("mailto:alice@example.com").unwrap();
        assert_eq!(p.scheme.as_deref(), Some("mailto"));
        assert_eq!(p.authority, None);
        assert_eq!(p.host, None);
        assert_eq!(p.path, "alice@example.com");
    }

    #[test]
    fn relative_reference_has_no_scheme() {
        let p = parse("/search?q=rust").unwrap();
        assert_eq!(p.scheme, None);
        assert!(!p.is_absolute);
        assert_eq!(p.path, "/search");
        assert_eq!(p.query_params[0].key, "q");
        assert_eq!(p.query_params[0].value.as_deref(), Some("rust"));
    }

    #[test]
    fn userinfo_without_password() {
        let p = parse("ftp://user@host.example/file").unwrap();
        assert_eq!(p.username.as_deref(), Some("user"));
        assert_eq!(p.password, None);
        assert_eq!(p.host.as_deref(), Some("host.example"));
    }

    #[test]
    fn bare_query_key_without_value() {
        let p = parse("https://x.io/?flag&k=v").unwrap();
        assert_eq!(p.query_params[0].key, "flag");
        assert_eq!(p.query_params[0].value, None);
        assert_eq!(p.query_params[1].key, "k");
        assert_eq!(p.query_params[1].value.as_deref(), Some("v"));
    }

    #[test]
    fn empty_query_value() {
        let p = parse("https://x.io/?k=").unwrap();
        assert_eq!(p.query_params[0].key, "k");
        assert_eq!(p.query_params[0].value.as_deref(), Some(""));
    }

    #[test]
    fn fragment_only() {
        let p = parse("https://x.io/page#section-2").unwrap();
        assert_eq!(p.fragment.as_deref(), Some("section-2"));
        assert_eq!(p.query, None);
    }

    #[test]
    fn duplicate_query_keys_preserved() {
        let p = parse("https://x.io/?a=1&a=2&a=3").unwrap();
        assert_eq!(p.query_params.len(), 3);
        assert_eq!(p.query_params[2].value.as_deref(), Some("3"));
    }

    #[test]
    fn host_only_no_path() {
        let p = parse("https://example.com").unwrap();
        assert_eq!(p.host.as_deref(), Some("example.com"));
        assert_eq!(p.path, "");
        assert_eq!(p.path_decoded, None);
    }

    #[test]
    fn file_url_with_drive_letter() {
        let p = parse("file:///C:/Users/me").unwrap();
        assert_eq!(p.scheme.as_deref(), Some("file"));
        assert_eq!(p.authority.as_deref(), Some(""));
        assert_eq!(p.host, None);
        assert_eq!(p.path, "/C:/Users/me");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(parse("   ").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let p = parse("  https://x.io/p  ").unwrap();
        assert_eq!(p.host.as_deref(), Some("x.io"));
        assert_eq!(p.path, "/p");
    }

    #[test]
    fn run_emits_valid_json() {
        let j = run("https://ex.com:443/p?x=1#f").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["scheme"], "https");
        assert_eq!(v["port"], 443);
        assert_eq!(v["fragment"], "f");
    }

    #[test]
    fn render_is_human_readable() {
        let out = render("https://user@ex.com/p?x=1#f").unwrap();
        assert!(out.contains("Scheme:"));
        assert!(out.contains("https"));
        assert!(out.contains("Username:"));
        assert!(out.contains("Fragment:"));
    }

    #[test]
    fn semicolon_separates_query_params() {
        let p = parse("https://x.io/?a=1;b=2").unwrap();
        assert_eq!(p.query_params.len(), 2);
        assert_eq!(p.query_params[1].key, "b");
    }

    #[test]
    fn path_segments_filename_and_extension() {
        let p = parse("https://ex.com/docs/2024/report%20final.pdf").unwrap();
        assert_eq!(p.path_segments, vec!["docs", "2024", "report final.pdf"]);
        assert_eq!(p.filename.as_deref(), Some("report final.pdf"));
        assert_eq!(p.file_extension.as_deref(), Some("pdf"));
    }

    #[test]
    fn trailing_slash_is_a_directory_not_a_file() {
        let p = parse("https://ex.com/a/b/").unwrap();
        assert_eq!(p.path_segments, vec!["a", "b"]);
        assert_eq!(p.filename, None);
        assert_eq!(p.file_extension, None);
    }

    #[test]
    fn dotfile_has_no_extension() {
        let p = parse("https://ex.com/.gitignore").unwrap();
        assert_eq!(p.filename.as_deref(), Some(".gitignore"));
        // leading-dot file: stem is empty, so no extension.
        assert_eq!(p.file_extension, None);
    }

    #[test]
    fn origin_present_only_with_scheme_and_authority() {
        // Origin drops userinfo and uses the normalized (lowercased) host.
        let p = parse("https://user@Ex.COM:8080/x").unwrap();
        assert_eq!(p.origin.as_deref(), Some("https://ex.com:8080"));
        // relative reference: no origin.
        let r = parse("/just/a/path").unwrap();
        assert_eq!(r.origin, None);
        // mailto has a scheme but no authority: no origin.
        let m = parse("mailto:a@b.com").unwrap();
        assert_eq!(m.origin, None);
    }
}
