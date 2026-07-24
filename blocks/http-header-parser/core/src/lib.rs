//! http-header-parser core — parse a raw HTTP request or response **header
//! block** into a structured, case-normalized MAP of `name → value`.
//!
//! Unlike a full-message parser, this focuses on just the headers: a leading
//! request/status line is optional (captured separately when present), header
//! names are folded case-insensitively and re-cased to a chosen style
//! (canonical Title-Case, lowercase, uppercase, or first-seen original), and
//! repeated headers are combined per a chosen policy. `Set-Cookie` is never
//! comma-joined (RFC 6265) — under the default `combine` policy it stays a
//! JSON array. Pure-Rust, no wafer/wasm-bindgen deps.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

/// How to re-case header names in the output map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Canonical HTTP Title-Case, e.g. `content-type` → `Content-Type`.
    Canonical,
    /// All lowercase, e.g. `Content-Type` → `content-type`.
    Lower,
    /// All uppercase, e.g. `Content-Type` → `CONTENT-TYPE`.
    Upper,
    /// Keep the casing exactly as first seen in the input.
    Original,
}

impl Case {
    pub fn parse(s: &str) -> Result<Case, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "canonical" => Ok(Case::Canonical),
            "lower" => Ok(Case::Lower),
            "upper" => Ok(Case::Upper),
            "original" => Ok(Case::Original),
            other => Err(format!(
                "unknown case '{other}' — use canonical, lower, upper, or original"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Case::Canonical => "canonical",
            Case::Lower => "lower",
            Case::Upper => "upper",
            Case::Original => "original",
        }
    }
}

/// How to fold repeated header lines that share a (case-insensitive) name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicates {
    /// Join values with `, ` (RFC 7230 §3.2.2); `Set-Cookie` stays an array.
    Combine,
    /// Keep every occurrence as a JSON array of strings.
    List,
    /// Keep only the first occurrence.
    First,
    /// Keep only the last occurrence.
    Last,
}

impl Duplicates {
    pub fn parse(s: &str) -> Result<Duplicates, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "combine" => Ok(Duplicates::Combine),
            "list" => Ok(Duplicates::List),
            "first" => Ok(Duplicates::First),
            "last" => Ok(Duplicates::Last),
            other => Err(format!(
                "unknown duplicates policy '{other}' — use combine, list, first, or last"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Duplicates::Combine => "combine",
            Duplicates::List => "list",
            Duplicates::First => "first",
            Duplicates::Last => "last",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Parsed {
    /// "request", "response", or "headers" (no start line detected).
    pub kind: String,
    /// The leading request or status line, if the block began with one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<String>,
    /// The case style applied to the header names.
    pub case: String,
    /// The duplicate-folding policy applied.
    pub duplicate_policy: String,
    /// The case-normalized header map, in first-seen order. A value is a string,
    /// or a JSON array when duplicates are listed / Set-Cookie is combined.
    pub headers: IndexMap<String, Value>,
    /// Number of distinct header names in the map.
    pub count: usize,
    /// Number of header lines parsed (before duplicate folding).
    pub line_count: usize,
    /// Header names (in the chosen case) that appeared more than once.
    pub duplicates: Vec<String>,
}

/// Canonical HTTP Title-Case for a already-lowercased header name. Most names
/// are per-segment capitalized (`x-frame-options` → `X-Frame-Options`); a few
/// well-known headers use non-standard casing and are special-cased.
fn canonical(lower: &str) -> String {
    match lower {
        "etag" => return "ETag".to_string(),
        "www-authenticate" => return "WWW-Authenticate".to_string(),
        "content-md5" => return "Content-MD5".to_string(),
        "te" => return "TE".to_string(),
        "dnt" => return "DNT".to_string(),
        "x-xss-protection" => return "X-XSS-Protection".to_string(),
        _ => {}
    }
    lower
        .split('-')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Re-case `original` (as seen) / `lower` per the chosen style.
fn display_name(case: Case, lower: &str, original: &str) -> String {
    match case {
        Case::Canonical => canonical(lower),
        Case::Lower => lower.to_string(),
        Case::Upper => lower.to_ascii_uppercase(),
        Case::Original => original.to_string(),
    }
}

/// Detect a leading start line. Returns the message kind if the line is a
/// request line (`METHOD target HTTP/x.y`) or a status line (`HTTP/x.y code …`),
/// else `None` (it's treated as a header).
fn start_line_kind(line: &str) -> Option<&'static str> {
    let l = line.trim();
    let first = l.split(' ').next().unwrap_or("");
    // A header line's first token ends the name at ':', so it can't be a start line.
    if first.contains(':') {
        return None;
    }
    if first.to_ascii_uppercase().starts_with("HTTP/") {
        return Some("response");
    }
    let parts: Vec<&str> = l.splitn(3, ' ').collect();
    if parts.len() == 3 && parts[2].trim().to_ascii_uppercase().starts_with("HTTP/") {
        return Some("request");
    }
    None
}

/// Parse a raw HTTP header block into a case-normalized map.
pub fn parse(input: &str, case: Case, dup: Duplicates) -> Result<Parsed, String> {
    let trimmed = input.trim_start_matches('\u{feff}');
    let trimmed = trimmed.trim_start_matches(['\r', '\n']);
    if trimmed.trim().is_empty() {
        return Err(
            "input is empty — paste an HTTP header block (one 'Name: value' per line)".into(),
        );
    }

    // Normalize newlines: split on LF, drop a trailing CR from each line.
    let lines: Vec<&str> = trimmed.split('\n').map(|l| l.trim_end_matches('\r')).collect();

    // Optional leading request/status line.
    let mut idx = 0usize;
    let mut kind = "headers".to_string();
    let mut start_line: Option<String> = None;
    if let Some(k) = lines.first().and_then(|l| start_line_kind(l)) {
        kind = k.to_string();
        start_line = Some(lines[0].trim().to_string());
        idx = 1;
    }

    // Parse the header lines (stop at the first blank line — the head/body split).
    // Store first-seen order of lower-cased names, the display name, and every value.
    let mut order: Vec<String> = Vec::new();
    let mut values: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut display: IndexMap<String, String> = IndexMap::new();
    let mut line_count = 0usize;
    let mut last_key: Option<String> = None;

    while idx < lines.len() {
        let line = lines[idx];
        idx += 1;
        if line.trim().is_empty() {
            break; // blank line ends the header block
        }
        // Obsolete line folding: a continuation line begins with whitespace.
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &last_key {
                if let Some(vs) = values.get_mut(k) {
                    if let Some(last) = vs.last_mut() {
                        last.push(' ');
                        last.push_str(line.trim());
                        continue;
                    }
                }
            }
            return Err(format!(
                "line '{line}' is a folded continuation but has no preceding header"
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("line '{line}' is not a 'Name: value' header (missing ':')"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("line '{line}' has an empty header name"));
        }
        let value = value.trim().to_string();
        let lower = name.to_ascii_lowercase();
        if !values.contains_key(&lower) {
            order.push(lower.clone());
            values.insert(lower.clone(), Vec::new());
            display.insert(lower.clone(), name.to_string());
        }
        values.get_mut(&lower).unwrap().push(value);
        line_count += 1;
        last_key = Some(lower);
    }

    if order.is_empty() {
        return Err("no headers found — each header must be on its own 'Name: value' line".into());
    }

    // Build the output map + the duplicates list, preserving first-seen order.
    let mut headers: IndexMap<String, Value> = IndexMap::new();
    let mut duplicates: Vec<String> = Vec::new();
    for lower in &order {
        let vals = &values[lower];
        let name = display_name(case, lower, &display[lower]);
        if vals.len() > 1 {
            duplicates.push(name.clone());
        }
        let value = build_value(lower, vals, dup);
        headers.insert(name, value);
    }

    Ok(Parsed {
        kind,
        start_line,
        case: case.label().to_string(),
        duplicate_policy: dup.label().to_string(),
        count: headers.len(),
        line_count,
        duplicates,
        headers,
    })
}

/// Fold the values collected for one header name into the output value.
fn build_value(lower: &str, vals: &[String], dup: Duplicates) -> Value {
    let as_array = || Value::Array(vals.iter().cloned().map(Value::String).collect());
    match dup {
        Duplicates::First => Value::String(vals[0].clone()),
        Duplicates::Last => Value::String(vals[vals.len() - 1].clone()),
        Duplicates::List => as_array(),
        // Set-Cookie must never be comma-joined (RFC 6265) — keep it a list.
        Duplicates::Combine if lower == "set-cookie" => as_array(),
        Duplicates::Combine => Value::String(vals.join(", ")),
    }
}

/// Parse and return pretty JSON (chat / programmatic / page surface).
pub fn run(input: &str, case: &str, duplicates: &str) -> Result<String, String> {
    let parsed = parse(input, Case::parse(case)?, Duplicates::parse(duplicates)?)?;
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(input: &str) -> Parsed {
        parse(input, Case::Canonical, Duplicates::Combine).unwrap()
    }

    #[test]
    fn parses_bare_header_block() {
        let m = p("content-type: application/json\nX-Custom: hi\n");
        assert_eq!(m.kind, "headers");
        assert!(m.start_line.is_none());
        assert_eq!(m.count, 2);
        assert_eq!(m.line_count, 2);
        assert_eq!(m.headers["Content-Type"], Value::String("application/json".into()));
        assert_eq!(m.headers["X-Custom"], Value::String("hi".into()));
    }

    #[test]
    fn canonicalizes_and_folds_case_insensitively() {
        let m = p("content-type: application/json\nX-CUSTOM: a\nx-custom: b\n");
        // X-CUSTOM and x-custom fold into one canonical key, values combined.
        assert_eq!(m.count, 2);
        assert_eq!(m.headers["X-Custom"], Value::String("a, b".into()));
        assert_eq!(m.duplicates, vec!["X-Custom".to_string()]);
    }

    #[test]
    fn detects_request_start_line() {
        let m = p("GET /index.html?q=1 HTTP/1.1\nHost: example.com\nAccept: */*\n");
        assert_eq!(m.kind, "request");
        assert_eq!(m.start_line.as_deref(), Some("GET /index.html?q=1 HTTP/1.1"));
        assert_eq!(m.count, 2);
        assert_eq!(m.headers["Host"], Value::String("example.com".into()));
    }

    #[test]
    fn detects_response_start_line() {
        let m = p("HTTP/1.1 200 OK\nContent-Type: text/html\n");
        assert_eq!(m.kind, "response");
        assert_eq!(m.start_line.as_deref(), Some("HTTP/1.1 200 OK"));
        assert_eq!(m.count, 1);
    }

    #[test]
    fn set_cookie_stays_a_list_under_combine() {
        let m = p("Set-Cookie: id=1; Path=/\nSet-Cookie: s=xyz; HttpOnly\nCache-Control: max-age=60\n");
        assert_eq!(
            m.headers["Set-Cookie"],
            Value::Array(vec![
                Value::String("id=1; Path=/".into()),
                Value::String("s=xyz; HttpOnly".into()),
            ])
        );
        assert_eq!(m.headers["Cache-Control"], Value::String("max-age=60".into()));
    }

    #[test]
    fn list_policy_makes_every_value_an_array() {
        let m = parse("A: 1\nA: 2\nB: 3\n", Case::Lower, Duplicates::List).unwrap();
        assert_eq!(
            m.headers["a"],
            Value::Array(vec![Value::String("1".into()), Value::String("2".into())])
        );
        assert_eq!(m.headers["b"], Value::Array(vec![Value::String("3".into())]));
    }

    #[test]
    fn first_and_last_policies() {
        let first = parse("A: 1\nA: 2\n", Case::Upper, Duplicates::First).unwrap();
        assert_eq!(first.headers["A"], Value::String("1".into()));
        let last = parse("A: 1\nA: 2\n", Case::Upper, Duplicates::Last).unwrap();
        assert_eq!(last.headers["A"], Value::String("2".into()));
    }

    #[test]
    fn upper_and_lower_and_original_casing() {
        assert_eq!(
            parse("Content-Type: x\n", Case::Upper, Duplicates::Combine).unwrap().headers.keys().next().unwrap(),
            "CONTENT-TYPE"
        );
        assert_eq!(
            parse("Content-Type: x\n", Case::Lower, Duplicates::Combine).unwrap().headers.keys().next().unwrap(),
            "content-type"
        );
        assert_eq!(
            parse("cOnTeNt-TyPe: x\n", Case::Original, Duplicates::Combine).unwrap().headers.keys().next().unwrap(),
            "cOnTeNt-TyPe"
        );
    }

    #[test]
    fn canonical_special_cases() {
        assert_eq!(canonical("etag"), "ETag");
        assert_eq!(canonical("www-authenticate"), "WWW-Authenticate");
        assert_eq!(canonical("x-xss-protection"), "X-XSS-Protection");
        assert_eq!(canonical("content-security-policy"), "Content-Security-Policy");
    }

    #[test]
    fn value_with_colons_is_preserved() {
        let m = p("Location: https://ex.com:8443/a?x=1\n");
        assert_eq!(m.headers["Location"], Value::String("https://ex.com:8443/a?x=1".into()));
    }

    #[test]
    fn folds_continuation_lines() {
        let m = p("X-Long: part1\n  part2\n");
        assert_eq!(m.count, 1);
        assert_eq!(m.headers["X-Long"], Value::String("part1 part2".into()));
    }

    #[test]
    fn stops_at_blank_line() {
        let m = p("Server: nginx\n\nthis is body, not a header\n");
        assert_eq!(m.count, 1);
        assert_eq!(m.headers["Server"], Value::String("nginx".into()));
    }

    #[test]
    fn tolerates_crlf() {
        let m = p("Host: x\r\nAccept: y\r\n");
        assert_eq!(m.count, 2);
        assert_eq!(m.headers["Host"], Value::String("x".into()));
    }

    #[test]
    fn run_emits_json() {
        let j = run("content-type: text/html\nSet-Cookie: a=1\nSet-Cookie: b=2\n", "canonical", "combine").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["kind"], "headers");
        assert_eq!(v["case"], "canonical");
        assert_eq!(v["headers"]["Content-Type"], "text/html");
        assert_eq!(v["headers"]["Set-Cookie"][0], "a=1");
        assert_eq!(v["count"], 2);
    }

    #[test]
    fn errors_on_empty() {
        assert!(parse("", Case::Canonical, Duplicates::Combine).is_err());
        assert!(parse("   \r\n\r\n", Case::Canonical, Duplicates::Combine).is_err());
    }

    #[test]
    fn errors_on_line_without_colon() {
        assert!(parse("this has no colon\n", Case::Canonical, Duplicates::Combine).is_err());
    }

    #[test]
    fn errors_on_empty_name() {
        assert!(parse(": value\n", Case::Canonical, Duplicates::Combine).is_err());
    }

    #[test]
    fn errors_on_unknown_options() {
        assert!(Case::parse("sideways").is_err());
        assert!(Duplicates::parse("merge").is_err());
    }
}
