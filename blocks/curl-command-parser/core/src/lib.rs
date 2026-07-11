//! gizza-ai/curl-command-parser core — parse a `curl` command line into a
//! structured HTTP request (method, URL, query params, headers, body, auth,
//! cookies, flags) and rebuild a clean, canonical `curl` command from the same
//! input. Pure-Rust, no wafer/wasm-bindgen deps; shared by the chat block and
//! the standalone page.
//!
//! The parser first tokenizes the command with a POSIX-ish shell tokenizer
//! (single quotes literal; double quotes with `\"`/`\\`/`\$`/backtick/newline
//! escapes; backslash escapes outside quotes; `\`+newline line-continuations),
//! strips a leading `curl`, then walks the tokens recognising the common
//! HTTP-related flags:
//!
//! | flag | meaning |
//! |------|---------|
//! | `-X` / `--request` | HTTP method |
//! | `-H` / `--header` | request header (repeatable) |
//! | `-d` / `--data` / `--data-ascii` | request body (newlines stripped) |
//! | `--data-raw` / `--data-binary` | request body (verbatim) |
//! | `--data-urlencode` | body field, percent-encoded |
//! | `-F` / `--form` | multipart form field (repeatable) |
//! | `-b` / `--cookie` | cookies (`k=v; k2=v2`) |
//! | `-u` / `--user` | HTTP basic auth (`user:password`) |
//! | `-A` / `--user-agent`, `-e` / `--referer` | UA / referer |
//! | `--url` | explicit request URL |
//! | `-G` / `--get` | send `-d` data as the query string |
//! | `-I` / `--head` | HEAD request |
//! | `-k` / `--insecure`, `-L` / `--location`, `--compressed` | boolean flags |

use serde::Serialize;

/// A generic decoded `key = value` pair (headers, query params, cookies, forms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

/// Decoded HTTP basic-auth credentials from `-u user:password`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BasicAuth {
    /// The username (before the first `:`).
    pub user: String,
    /// The password (after the first `:`); `None` when no `:` was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// The structured result of parsing a `curl` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ParsedCurl {
    /// The HTTP method, upper-cased. Explicit `-X` wins; otherwise inferred:
    /// HEAD for `-I`, POST when a body/form is present, else GET.
    pub method: String,
    /// The request URL, as written (from a positional argument or `--url`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Query parameters split out of the URL (percent-decoded). With `-G`, the
    /// `-d` data fields are appended here instead of forming a body.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub query_params: Vec<KeyValue>,
    /// Request headers from `-H`, in order (name trimmed, value trimmed).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<KeyValue>,
    /// The effective `Content-Type`: an explicit header wins, otherwise inferred
    /// (`application/x-www-form-urlencoded` for `-d`, `multipart/form-data` for `-F`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The request body assembled from `-d`/`--data*` fields (joined with `&`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// How the body was supplied: `data`, `data-raw`, `data-binary`, or
    /// `data-urlencode` (the first data flag seen). `None` when there is no body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_kind: Option<String>,
    /// A `@file` reference given to `-d`/`--data-binary`/`-F` — the local file
    /// whose contents curl would read (we cannot read it here).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub file_refs: Vec<String>,
    /// Multipart form fields from `-F`, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub form_fields: Vec<KeyValue>,
    /// Cookies from `-b`, parsed from a `k=v; k2=v2` string (a bare cookie-jar
    /// filename lands in `file_refs`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cookies: Vec<KeyValue>,
    /// HTTP basic auth from `-u`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth: Option<BasicAuth>,
    /// The `-A`/`--user-agent` value, if given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// The `-e`/`--referer` value, if given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
    /// `-k` / `--insecure`: skip TLS certificate verification.
    #[serde(skip_serializing_if = "is_false")]
    pub insecure: bool,
    /// `-L` / `--location`: follow HTTP redirects.
    #[serde(skip_serializing_if = "is_false")]
    pub follow_redirects: bool,
    /// `--compressed`: request a compressed response.
    #[serde(skip_serializing_if = "is_false")]
    pub compressed: bool,
    /// `-I` / `--head`: fetch headers only (HEAD).
    #[serde(skip_serializing_if = "is_false")]
    pub head: bool,
    /// `-G` / `--get`: send the `-d` data as the query string.
    #[serde(skip_serializing_if = "is_false")]
    pub get: bool,
    /// Any other flags that were recognised as options but not modelled above
    /// (e.g. `-s`, `-v`, `--fail`), recorded verbatim for transparency.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_flags: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------------------
// Shell-ish tokenizer
// ---------------------------------------------------------------------------

/// Tokenize a command line the way a POSIX shell would for the common cases:
/// whitespace separates tokens; single quotes are literal; double quotes allow
/// `\"`, `\\`, `\$`, `` \` `` and `\<newline>` escapes; a backslash outside
/// quotes escapes the next character; and a backslash directly before a newline
/// is a line continuation. Returns `Err` on an unterminated quote.
pub fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut has_token = false; // distinguishes an empty quoted token from no token
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if has_token {
                    tokens.push(std::mem::take(&mut cur));
                    has_token = false;
                }
                i += 1;
            }
            '\'' => {
                has_token = true;
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == '\'' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    cur.push(chars[i]);
                    i += 1;
                }
                if !closed {
                    return Err("unterminated single quote in command".into());
                }
            }
            '"' => {
                has_token = true;
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let d = chars[i];
                    if d == '"' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if d == '\\' && i + 1 < chars.len() {
                        let n = chars[i + 1];
                        if matches!(n, '"' | '\\' | '$' | '`') {
                            cur.push(n);
                            i += 2;
                            continue;
                        }
                        if n == '\n' {
                            // line continuation inside double quotes
                            i += 2;
                            continue;
                        }
                    }
                    cur.push(d);
                    i += 1;
                }
                if !closed {
                    return Err("unterminated double quote in command".into());
                }
            }
            '\\' => {
                if i + 1 < chars.len() {
                    let n = chars[i + 1];
                    if n == '\n' {
                        // line continuation — drop the backslash+newline
                        i += 2;
                        continue;
                    }
                    if n == '\r' && i + 2 < chars.len() && chars[i + 2] == '\n' {
                        i += 3;
                        continue;
                    }
                    has_token = true;
                    cur.push(n);
                    i += 2;
                } else {
                    // trailing backslash — keep it literal
                    has_token = true;
                    cur.push('\\');
                    i += 1;
                }
            }
            other => {
                has_token = true;
                cur.push(other);
                i += 1;
            }
        }
    }
    if has_token {
        tokens.push(cur);
    }
    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Percent-encoding helpers
// ---------------------------------------------------------------------------

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode with `+`→space (form/query style). Invalid escapes are kept
/// literal; bytes are decoded as UTF-8 lossily.
fn decode_form(s: &str) -> String {
    let replaced = s.replace('+', " ");
    let bytes = replaced.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
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

/// Percent-encode a value for `application/x-www-form-urlencoded` the way curl's
/// `--data-urlencode` does: keep the RFC-3986 unreserved set, escape the rest.
fn encode_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Flag helpers
// ---------------------------------------------------------------------------

/// A single decoded data field with its origin flag kind.
struct DataField {
    kind: &'static str, // "data" | "data-raw" | "data-binary" | "data-urlencode"
    raw: String,
}

/// Long option names that consume a value.
fn long_takes_value(name: &str) -> bool {
    matches!(
        name,
        "request"
            | "header"
            | "data"
            | "data-ascii"
            | "data-raw"
            | "data-binary"
            | "data-urlencode"
            | "form"
            | "form-string"
            | "url"
            | "user"
            | "cookie"
            | "user-agent"
            | "referer"
            | "output"
            | "connect-timeout"
            | "max-time"
            | "retry"
            | "proxy"
    )
}

/// Map a short option letter to its canonical long name; returns `(name, takes_value)`.
fn short_option(c: char) -> Option<(&'static str, bool)> {
    Some(match c {
        'X' => ("request", true),
        'H' => ("header", true),
        'd' => ("data", true),
        'F' => ("form", true),
        'u' => ("user", true),
        'b' => ("cookie", true),
        'A' => ("user-agent", true),
        'e' => ("referer", true),
        'o' => ("output", true),
        'G' => ("get", false),
        'I' => ("head", false),
        'k' => ("insecure", false),
        'L' => ("location", false),
        's' => ("silent", false),
        'S' => ("show-error", false),
        'v' => ("verbose", false),
        'f' => ("fail", false),
        'g' => ("globoff", false),
        '#' => ("progress-bar", false),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a `curl` command string into a [`ParsedCurl`].
pub fn parse(input: &str) -> Result<ParsedCurl, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty input: provide a curl command to parse".into());
    }
    let tokens = tokenize(trimmed)?;
    if tokens.is_empty() {
        return Err("empty input: provide a curl command to parse".into());
    }

    let mut p = ParsedCurl::default();
    let mut explicit_method: Option<String> = None;
    let mut data_fields: Vec<DataField> = Vec::new();

    let mut idx = 0;
    // Skip a leading "curl" argv[0].
    if idx < tokens.len() && (tokens[idx] == "curl" || tokens[idx].ends_with("/curl")) {
        idx += 1;
    }

    while idx < tokens.len() {
        let tok = tokens[idx].clone();
        idx += 1;

        if tok == "--" {
            // Everything after `--` is a positional (URL).
            while idx < tokens.len() {
                set_url(&mut p, &tokens[idx]);
                idx += 1;
            }
            break;
        }

        if let Some(long) = tok.strip_prefix("--") {
            let (name, inline_val) = match long.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (long.to_string(), None),
            };
            let value = if long_takes_value(&name) {
                match inline_val {
                    Some(v) => v,
                    None => {
                        if idx >= tokens.len() {
                            return Err(format!("option --{name} expects a value"));
                        }
                        let v = tokens[idx].clone();
                        idx += 1;
                        v
                    }
                }
            } else {
                String::new()
            };
            apply_option(
                &mut p,
                &name,
                &value,
                &mut explicit_method,
                &mut data_fields,
            );
            continue;
        }

        if tok.starts_with('-') && tok.len() > 1 {
            // A run of short options, e.g. -sSL or -XPOST or -d@file.
            let letters: Vec<char> = tok[1..].chars().collect();
            let mut j = 0;
            while j < letters.len() {
                let c = letters[j];
                match short_option(c) {
                    Some((name, takes_value)) => {
                        if takes_value {
                            // The rest of this token is the attached value; else
                            // the value is the next token.
                            let rest: String = letters[j + 1..].iter().collect();
                            let value = if !rest.is_empty() {
                                rest
                            } else if idx < tokens.len() {
                                let v = tokens[idx].clone();
                                idx += 1;
                                v
                            } else {
                                return Err(format!("option -{c} expects a value"));
                            };
                            apply_option(
                                &mut p,
                                name,
                                &value,
                                &mut explicit_method,
                                &mut data_fields,
                            );
                            j = letters.len(); // value consumed the remainder
                        } else {
                            apply_option(
                                &mut p,
                                name,
                                "",
                                &mut explicit_method,
                                &mut data_fields,
                            );
                            j += 1;
                        }
                    }
                    None => {
                        // Unknown short flag — record it and stop scanning this token.
                        p.other_flags.push(format!("-{c}"));
                        j += 1;
                    }
                }
            }
            continue;
        }

        // A positional argument — the URL.
        set_url(&mut p, &tok);
    }

    // --- Assemble the body / query / content-type ---------------------------
    let has_form = !p.form_fields.is_empty();

    if p.get {
        // -G moves data fields into the query string.
        for f in &data_fields {
            push_query_field(&mut p.query_params, &f.raw, f.kind);
        }
    } else if !data_fields.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        for f in &data_fields {
            if let Some(file) = f.raw.strip_prefix('@') {
                p.file_refs.push(file.to_string());
                continue;
            }
            let piece = match f.kind {
                // --data / --data-ascii strip embedded newlines like curl does.
                "data" => f.raw.replace(['\n', '\r'], ""),
                "data-urlencode" => urlencode_field(&f.raw),
                _ => f.raw.clone(), // data-raw / data-binary: verbatim
            };
            parts.push(piece);
        }
        if !parts.is_empty() {
            p.body = Some(parts.join("&"));
            // body_kind = the first data flag seen (only meaningful with a body).
            p.body_kind = Some(data_fields[0].kind.to_string());
        }
    }

    // Effective Content-Type: an explicit header wins.
    let explicit_ct = p
        .headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.clone());
    p.content_type = if let Some(ct) = explicit_ct {
        Some(ct)
    } else if has_form {
        Some("multipart/form-data".to_string())
    } else if p.body.is_some() && !p.get {
        Some("application/x-www-form-urlencoded".to_string())
    } else {
        None
    };

    // --- Method inference ---------------------------------------------------
    // curl treats any -d/--data* field as a POST even when it only referenced a
    // @file (so body ended up empty); key method inference off data presence.
    let has_data = !data_fields.is_empty();
    p.method = if let Some(m) = explicit_method {
        m.to_ascii_uppercase()
    } else if p.head {
        "HEAD".to_string()
    } else if (has_data || has_form) && !p.get {
        "POST".to_string()
    } else {
        "GET".to_string()
    };

    // --- Split query params out of the URL ----------------------------------
    if let Some(url) = &p.url {
        if let Some((_, q)) = url.split_once('?') {
            // Query params from the URL come first, before any -G data.
            let mut from_url = Vec::new();
            for pair in q.split('&').filter(|s| !s.is_empty()) {
                let (k, v) = match pair.split_once('=') {
                    Some((k, v)) => (decode_form(k), decode_form(v)),
                    None => (decode_form(pair), String::new()),
                };
                from_url.push(KeyValue { key: k, value: v });
            }
            let mut merged = from_url;
            merged.append(&mut p.query_params);
            p.query_params = merged;
        }
    }

    if p.url.is_none() && p.headers.is_empty() && p.body.is_none() && p.form_fields.is_empty() {
        return Err("no curl request found: expected at least a URL".into());
    }

    Ok(p)
}

/// Set the URL, but only the first positional/`--url` wins.
fn set_url(p: &mut ParsedCurl, v: &str) {
    if p.url.is_none() {
        p.url = Some(v.to_string());
    }
}

/// Push a `-G` / query data field (`name=value` or bare) as a decoded param.
fn push_query_field(out: &mut Vec<KeyValue>, raw: &str, kind: &str) {
    let raw = raw.strip_prefix('@').unwrap_or(raw);
    match raw.split_once('=') {
        Some((k, v)) => out.push(KeyValue {
            key: k.to_string(),
            value: if kind == "data-urlencode" {
                v.to_string()
            } else {
                decode_form(v)
            },
        }),
        None => out.push(KeyValue {
            key: raw.to_string(),
            value: String::new(),
        }),
    }
}

/// Encode a `--data-urlencode` field: `name=content` → `name=<pct>`, `=content`
/// or `content` → `<pct>`.
fn urlencode_field(raw: &str) -> String {
    if let Some((name, content)) = raw.split_once('=') {
        if name.is_empty() {
            encode_form(content)
        } else {
            format!("{name}={}", encode_form(content))
        }
    } else {
        encode_form(raw)
    }
}

/// Apply one normalised option (canonical long `name`) with its `value`.
fn apply_option(
    p: &mut ParsedCurl,
    name: &str,
    value: &str,
    method: &mut Option<String>,
    data: &mut Vec<DataField>,
) {
    match name {
        "request" => *method = Some(value.to_string()),
        "header" => {
            // "Name: Value"; a bare "Name;" means send an empty header.
            if let Some((k, v)) = value.split_once(':') {
                p.headers.push(KeyValue {
                    key: k.trim().to_string(),
                    value: v.trim().to_string(),
                });
            } else if let Some(k) = value.strip_suffix(';') {
                p.headers.push(KeyValue {
                    key: k.trim().to_string(),
                    value: String::new(),
                });
            } else {
                p.headers.push(KeyValue {
                    key: value.trim().to_string(),
                    value: String::new(),
                });
            }
        }
        "data" | "data-ascii" => data.push(DataField {
            kind: "data",
            raw: value.to_string(),
        }),
        "data-raw" => data.push(DataField {
            kind: "data-raw",
            raw: value.to_string(),
        }),
        "data-binary" => data.push(DataField {
            kind: "data-binary",
            raw: value.to_string(),
        }),
        "data-urlencode" => data.push(DataField {
            kind: "data-urlencode",
            raw: value.to_string(),
        }),
        "form" | "form-string" => {
            if let Some((k, v)) = value.split_once('=') {
                if let Some(file) = v.strip_prefix('@') {
                    p.file_refs.push(file.to_string());
                    p.form_fields.push(KeyValue {
                        key: k.to_string(),
                        value: format!("@{file}"),
                    });
                } else {
                    p.form_fields.push(KeyValue {
                        key: k.to_string(),
                        value: v.to_string(),
                    });
                }
            } else {
                p.form_fields.push(KeyValue {
                    key: value.to_string(),
                    value: String::new(),
                });
            }
        }
        "url" => set_url(p, value),
        "user" => {
            let (user, password) = match value.split_once(':') {
                Some((u, pw)) => (u.to_string(), Some(pw.to_string())),
                None => (value.to_string(), None),
            };
            p.basic_auth = Some(BasicAuth { user, password });
        }
        "cookie" => {
            // "k=v; k2=v2" pairs, or a bare cookie-jar filename.
            if value.contains('=') {
                for part in value.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    match part.split_once('=') {
                        Some((k, v)) => p.cookies.push(KeyValue {
                            key: k.trim().to_string(),
                            value: v.trim().to_string(),
                        }),
                        None => p.cookies.push(KeyValue {
                            key: part.to_string(),
                            value: String::new(),
                        }),
                    }
                }
            } else {
                p.file_refs.push(value.to_string());
            }
        }
        "user-agent" => p.user_agent = Some(value.to_string()),
        "referer" => p.referer = Some(value.to_string()),
        "get" => p.get = true,
        "head" => p.head = true,
        "insecure" => p.insecure = true,
        "location" => p.follow_redirects = true,
        "compressed" => p.compressed = true,
        // Recognised but not modelled — record for transparency.
        "output" => p.other_flags.push(format!("--output {value}")),
        "silent" => p.other_flags.push("--silent".into()),
        "show-error" => p.other_flags.push("--show-error".into()),
        "verbose" => p.other_flags.push("--verbose".into()),
        "fail" => p.other_flags.push("--fail".into()),
        "globoff" => p.other_flags.push("--globoff".into()),
        "progress-bar" => p.other_flags.push("--progress-bar".into()),
        "connect-timeout" | "max-time" | "retry" | "proxy" => {
            p.other_flags.push(format!("--{name} {value}"))
        }
        other => {
            if value.is_empty() {
                p.other_flags.push(format!("--{other}"));
            } else {
                p.other_flags.push(format!("--{other} {value}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rebuild
// ---------------------------------------------------------------------------

/// Single-quote a value for a POSIX shell: wrap in `'…'`, escaping any embedded
/// single quote as `'\''`. A value with no shell-special chars is left bare.
fn shell_quote(s: &str) -> String {
    let needs = s.is_empty()
        || s.chars().any(|c| {
            !matches!(c,
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '/' | ':' | '@' | '%' | '+' | ',' | '=')
        });
    if !needs {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Rebuild a clean, canonical multi-line `curl` command from a parsed request.
/// Deterministic ordering: method, URL, headers (in order), auth, cookies,
/// body/form, then boolean flags.
pub fn rebuild(p: &ParsedCurl) -> String {
    let mut lines: Vec<String> = Vec::new();

    // First line: `curl` + method (only when not a plain GET) + URL.
    let mut first = String::from("curl");
    if p.method != "GET" && !p.head {
        first.push_str(&format!(" -X {}", p.method));
    }
    if let Some(url) = &p.url {
        first.push(' ');
        first.push_str(&shell_quote(url));
    }
    lines.push(first);

    for h in &p.headers {
        let hv = if h.value.is_empty() {
            format!("{};", h.key)
        } else {
            format!("{}: {}", h.key, h.value)
        };
        lines.push(format!("-H {}", shell_quote(&hv)));
    }

    if let Some(a) = &p.basic_auth {
        let cred = match &a.password {
            Some(pw) => format!("{}:{}", a.user, pw),
            None => a.user.clone(),
        };
        lines.push(format!("-u {}", shell_quote(&cred)));
    }

    if !p.cookies.is_empty() {
        let jar = p
            .cookies
            .iter()
            .map(|c| format!("{}={}", c.key, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        lines.push(format!("-b {}", shell_quote(&jar)));
    }

    for f in &p.form_fields {
        let field = if f.value.is_empty() {
            f.key.clone()
        } else {
            format!("{}={}", f.key, f.value)
        };
        lines.push(format!("-F {}", shell_quote(&field)));
    }

    if let Some(body) = &p.body {
        let flag = match p.body_kind.as_deref() {
            Some("data-raw") => "--data-raw",
            Some("data-binary") => "--data-binary",
            _ => "--data",
        };
        lines.push(format!("{flag} {}", shell_quote(body)));
    }

    if p.head {
        lines.push("-I".to_string());
    }
    if p.follow_redirects {
        lines.push("-L".to_string());
    }
    if p.compressed {
        lines.push("--compressed".to_string());
    }
    if p.insecure {
        lines.push("-k".to_string());
    }
    if p.get && p.body.is_none() {
        lines.push("-G".to_string());
    }
    if let Some(ua) = &p.user_agent {
        lines.push(format!("-A {}", shell_quote(ua)));
    }
    if let Some(r) = &p.referer {
        lines.push(format!("-e {}", shell_quote(r)));
    }

    // Join with " \\\n  " continuations for readability.
    lines.join(" \\\n  ")
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Parse + return pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let parsed = parse(input)?;
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

/// Unified entry point shared by every surface (chat / CLI / page).
///
/// - `mode` (`"parse"` default | `"rebuild"`): direction.
/// - parse mode: `human=true` renders the aligned text view (page), `false`
///   returns pretty JSON (chat/CLI).
/// - rebuild mode: returns a clean canonical curl command (identical on every
///   surface).
pub fn dispatch(mode: &str, command: &str, human: bool) -> Result<String, String> {
    match mode.trim() {
        "" | "parse" => {
            if human {
                render(command)
            } else {
                run(command)
            }
        }
        "rebuild" => {
            let parsed = parse(command)?;
            Ok(rebuild(&parsed))
        }
        other => Err(format!(
            "invalid mode {other:?}: expected \"parse\" or \"rebuild\""
        )),
    }
}

/// Human-readable rendering of a parsed curl command (page parse mode).
pub fn render(input: &str) -> Result<String, String> {
    let p = parse(input)?;
    let mut out = String::new();
    let row = |out: &mut String, label: &str, val: &str| {
        out.push_str(&format!("{label:<16}{val}\n"));
    };
    row(&mut out, "Method:", &p.method);
    if let Some(url) = &p.url {
        row(&mut out, "URL:", url);
    }
    if let Some(ct) = &p.content_type {
        row(&mut out, "Content-Type:", ct);
    }
    if let Some(a) = &p.basic_auth {
        let pw = a.password.as_deref().unwrap_or("");
        row(&mut out, "Basic auth:", &format!("{} : {}", a.user, pw));
    }
    if let Some(ua) = &p.user_agent {
        row(&mut out, "User-Agent:", ua);
    }
    if let Some(r) = &p.referer {
        row(&mut out, "Referer:", r);
    }

    let flags = {
        let mut f: Vec<&str> = Vec::new();
        if p.head {
            f.push("head (-I)");
        }
        if p.follow_redirects {
            f.push("follow-redirects (-L)");
        }
        if p.insecure {
            f.push("insecure (-k)");
        }
        if p.compressed {
            f.push("compressed");
        }
        if p.get {
            f.push("get (-G)");
        }
        f
    };
    if !flags.is_empty() {
        row(&mut out, "Flags:", &flags.join(", "));
    }

    let list = |out: &mut String, label: &str, items: &[KeyValue], sep: &str| {
        if !items.is_empty() {
            out.push_str(&format!("\n{label} ({}):\n", items.len()));
            for kv in items {
                out.push_str(&format!("  {}{sep}{}\n", kv.key, kv.value));
            }
        }
    };
    list(&mut out, "Headers", &p.headers, ": ");
    list(&mut out, "Query params", &p.query_params, " = ");
    list(&mut out, "Cookies", &p.cookies, " = ");
    list(&mut out, "Form fields", &p.form_fields, " = ");

    if let Some(body) = &p.body {
        out.push_str(&format!(
            "\nBody ({}):\n  {body}\n",
            p.body_kind.as_deref().unwrap_or("data")
        ));
    }
    if !p.file_refs.is_empty() {
        out.push_str(&format!("\nFile references ({}):\n", p.file_refs.len()));
        for f in &p.file_refs {
            out.push_str(&format!("  @{f}\n"));
        }
    }
    if !p.other_flags.is_empty() {
        out.push_str(&format!("\nOther flags ({}):\n", p.other_flags.len()));
        for f in &p.other_flags {
            out.push_str(&format!("  {f}\n"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_handles_quotes_and_continuations() {
        let toks = tokenize("curl 'https://a.com' \\\n  -H \"X: y z\" -d 'a=1'").unwrap();
        assert_eq!(
            toks,
            vec!["curl", "https://a.com", "-H", "X: y z", "-d", "a=1"]
        );
    }

    #[test]
    fn tokenize_rejects_unterminated_quote() {
        assert!(tokenize("curl 'https://a.com").is_err());
    }

    #[test]
    fn parses_get_with_headers() {
        let p = parse("curl https://api.example.com/v1/items -H 'Accept: application/json' -H 'X-Token: abc'").unwrap();
        assert_eq!(p.method, "GET");
        assert_eq!(p.url.as_deref(), Some("https://api.example.com/v1/items"));
        assert_eq!(p.headers.len(), 2);
        assert_eq!(p.headers[0].key, "Accept");
        assert_eq!(p.headers[0].value, "application/json");
        assert_eq!(p.headers[1].key, "X-Token");
    }

    #[test]
    fn infers_post_from_body_and_content_type() {
        let p = parse("curl https://api.example.com -d 'name=gizza&x=1'").unwrap();
        assert_eq!(p.method, "POST");
        assert_eq!(p.body.as_deref(), Some("name=gizza&x=1"));
        assert_eq!(
            p.content_type.as_deref(),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(p.body_kind.as_deref(), Some("data"));
    }

    #[test]
    fn explicit_method_and_attached_short_flag() {
        // -XPOST attached, --data-raw verbatim, explicit content-type header wins.
        let p = parse("curl -XPOST https://h/api -H 'Content-Type: application/json' --data-raw '{\"a\":1}'").unwrap();
        assert_eq!(p.method, "POST");
        assert_eq!(p.body.as_deref(), Some("{\"a\":1}"));
        assert_eq!(p.body_kind.as_deref(), Some("data-raw"));
        assert_eq!(p.content_type.as_deref(), Some("application/json"));
    }

    #[test]
    fn parses_basic_auth_user_and_password() {
        let p = parse("curl -u alice:s3cret https://h/").unwrap();
        let a = p.basic_auth.unwrap();
        assert_eq!(a.user, "alice");
        assert_eq!(a.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn basic_auth_without_password() {
        let p = parse("curl --user bob https://h/").unwrap();
        let a = p.basic_auth.unwrap();
        assert_eq!(a.user, "bob");
        assert!(a.password.is_none());
    }

    #[test]
    fn splits_query_params_from_url() {
        let p = parse("curl 'https://h/s?q=hello+world&page=2'").unwrap();
        assert_eq!(p.query_params.len(), 2);
        assert_eq!(p.query_params[0].key, "q");
        assert_eq!(p.query_params[0].value, "hello world");
        assert_eq!(p.query_params[1].key, "page");
        assert_eq!(p.query_params[1].value, "2");
    }

    #[test]
    fn combined_boolean_short_flags() {
        let p = parse("curl -sSLk https://h/").unwrap();
        assert!(p.follow_redirects);
        assert!(p.insecure);
        assert!(p.other_flags.iter().any(|f| f == "--silent"));
        assert!(p.other_flags.iter().any(|f| f == "--show-error"));
    }

    #[test]
    fn head_flag_sets_method() {
        let p = parse("curl -I https://h/").unwrap();
        assert_eq!(p.method, "HEAD");
        assert!(p.head);
    }

    #[test]
    fn get_flag_moves_data_to_query() {
        let p = parse("curl -G https://h/search -d q=rust -d limit=10").unwrap();
        assert_eq!(p.method, "GET");
        assert!(p.body.is_none());
        assert_eq!(p.query_params.len(), 2);
        assert_eq!(p.query_params[0].key, "q");
        assert_eq!(p.query_params[0].value, "rust");
        assert_eq!(p.query_params[1].key, "limit");
    }

    #[test]
    fn cookies_and_form_and_compressed() {
        let p = parse("curl https://h/upload -b 'sid=abc; theme=dark' -F 'file=@photo.png' -F 'name=pic' --compressed").unwrap();
        assert_eq!(p.cookies.len(), 2);
        assert_eq!(p.cookies[0].key, "sid");
        assert_eq!(p.cookies[1].value, "dark");
        assert_eq!(p.form_fields.len(), 2);
        assert_eq!(p.form_fields[0].value, "@photo.png");
        assert!(p.file_refs.iter().any(|f| f == "photo.png"));
        assert!(p.compressed);
        assert_eq!(p.method, "POST");
        assert_eq!(p.content_type.as_deref(), Some("multipart/form-data"));
    }

    #[test]
    fn data_urlencode_encodes() {
        let p = parse("curl https://h/ --data-urlencode 'q=a b&c'").unwrap();
        assert_eq!(p.body.as_deref(), Some("q=a%20b%26c"));
        assert_eq!(p.body_kind.as_deref(), Some("data-urlencode"));
    }

    #[test]
    fn at_file_data_becomes_file_ref() {
        let p = parse("curl https://h/ --data-binary @payload.bin").unwrap();
        assert!(p.body.is_none());
        assert!(p.file_refs.iter().any(|f| f == "payload.bin"));
        assert_eq!(p.method, "POST");
    }

    #[test]
    fn user_agent_and_referer_and_url_flag() {
        let p = parse("curl --url https://h/ -A 'gizza/1.0' -e 'https://ref/'").unwrap();
        assert_eq!(p.url.as_deref(), Some("https://h/"));
        assert_eq!(p.user_agent.as_deref(), Some("gizza/1.0"));
        assert_eq!(p.referer.as_deref(), Some("https://ref/"));
    }

    #[test]
    fn rejects_empty() {
        assert!(parse("   ").is_err());
    }

    #[test]
    fn rejects_command_without_url() {
        assert!(parse("curl -k").is_err());
    }

    #[test]
    fn run_emits_valid_json() {
        let j = run("curl https://h/ -H 'A: b'").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["method"], "GET");
        assert_eq!(v["url"], "https://h/");
        assert_eq!(v["headers"][0]["key"], "A");
    }

    #[test]
    fn rebuild_produces_clean_command() {
        let p = parse("curl -XPOST 'https://api.example.com/items' -H 'Accept: application/json' -d 'a=1'").unwrap();
        let cmd = rebuild(&p);
        assert_eq!(
            cmd,
            "curl -X POST https://api.example.com/items \\\n  -H 'Accept: application/json' \\\n  --data a=1"
        );
    }

    #[test]
    fn rebuild_get_omits_method() {
        let p = parse("curl https://h/ -H 'X: y'").unwrap();
        let cmd = rebuild(&p);
        assert_eq!(cmd, "curl https://h/ \\\n  -H 'X: y'");
    }

    #[test]
    fn rebuild_round_trips_through_parse() {
        let original = "curl -X PUT 'https://h/r' -H 'Content-Type: application/json' -u u:p --data-raw '{\"k\":\"v\"}' -L --compressed";
        let p1 = parse(original).unwrap();
        let rebuilt = rebuild(&p1);
        let p2 = parse(&rebuilt).unwrap();
        assert_eq!(p1.method, p2.method);
        assert_eq!(p1.url, p2.url);
        assert_eq!(p1.headers, p2.headers);
        assert_eq!(p1.body, p2.body);
        assert_eq!(p1.basic_auth, p2.basic_auth);
        assert!(p2.follow_redirects && p2.compressed);
    }

    #[test]
    fn dispatch_parse_json_rebuild_and_bad_mode() {
        let j = dispatch("parse", "curl https://h/", false).unwrap();
        assert!(j.contains("\"method\": \"GET\""));
        let cmd = dispatch("rebuild", "curl -d x=1 https://h/", false).unwrap();
        assert!(cmd.starts_with("curl -X POST https://h/"));
        assert!(dispatch("frobnicate", "curl https://h/", false).is_err());
    }

    #[test]
    fn render_is_human_readable() {
        let out =
            render("curl -XPOST 'https://h/api' -H 'Accept: application/json' -d 'a=1&b=2'").unwrap();
        assert!(out.contains("Method:"));
        assert!(out.contains("POST"));
        assert!(out.contains("URL:"));
        assert!(out.contains("Headers (1):"));
        assert!(out.contains("Body (data):"));
    }
}
