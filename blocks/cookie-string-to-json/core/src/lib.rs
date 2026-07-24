//! cookie-string-to-json core — parse a raw HTTP request `Cookie:` header
//! string (`name1=value1; name2=value2; …`) into JSON. Splits on `;` (and
//! newlines), trims whitespace, strips a pasted `Cookie:`/`Set-Cookie:` header
//! name, unwraps RFC 6265 double-quoted values, and (by default) percent-decodes
//! names and values. Two output shapes: a name→value object (repeated names
//! collapse into an array), or an ordered array of `{name, value}` objects.
//! Pure-Rust, no wafer/wasm-bindgen deps. Shared by the chat block and the page.

use serde_json::{Map, Value};

/// One decoded cookie, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

/// Decode a hex nibble.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a string. Invalid `%` escapes are left literal (lenient — a
/// stray `%` shouldn't error). Unlike a URL query string, a `+` is NOT decoded
/// to a space here: cookie values are not `application/x-www-form-urlencoded`,
/// so `+` is a literal plus. Bytes are decoded as UTF-8 lossily so non-UTF-8
/// sequences degrade to U+FFFD.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip a pasted `Cookie:` / `Set-Cookie:` header-name prefix (case-insensitive)
/// and surrounding whitespace, so the whole header line can be pasted as-is.
fn normalize(input: &str) -> &str {
    let s = input.trim();
    let lower = s.to_ascii_lowercase();
    for prefix in ["set-cookie:", "cookie:"] {
        if lower.starts_with(prefix) {
            return s[prefix.len()..].trim_start();
        }
    }
    s
}

/// If the value is wrapped in a matched pair of double quotes (RFC 6265
/// `cookie-value = DQUOTE *cookie-octet DQUOTE`), unwrap them.
fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Parse a Cookie header string into an ordered list of cookies (duplicates
/// preserved). Splits on `;` and newlines; empty segments and segments with an
/// empty name are skipped. A segment with no `=` becomes a name with an empty
/// value. When `decode`, names and values are percent-decoded. Always succeeds —
/// empty input yields an empty list rather than an error.
pub fn parse(input: &str, decode: bool) -> Vec<Cookie> {
    let s = normalize(input);
    let mut cookies = Vec::new();
    for part in s.split([';', '\n']) {
        let seg = part.trim();
        if seg.is_empty() {
            continue;
        }
        let (raw_name, raw_val) = match seg.split_once('=') {
            Some((n, v)) => (n.trim(), unquote(v.trim())),
            None => (seg, ""),
        };
        if raw_name.is_empty() {
            continue;
        }
        let (name, value) = if decode {
            (percent_decode(raw_name), percent_decode(raw_val))
        } else {
            (raw_name.to_string(), raw_val.to_string())
        };
        cookies.push(Cookie { name, value });
    }
    cookies
}

/// If `key` already exists, turn it into / append to an array; else set scalar.
fn merge_scalar(map: &mut Map<String, Value>, key: &str, value: Value) {
    match map.get_mut(key) {
        None => {
            map.insert(key.to_string(), value);
        }
        Some(existing) => {
            if let Value::Array(arr) = existing {
                arr.push(value);
            } else {
                let prev = existing.take();
                *existing = Value::Array(vec![prev, value]);
            }
        }
    }
}

/// Build the JSON value for the chosen output shape. `"object"` → a name→value
/// object in source order, where a repeated name collapses into an array of its
/// values. `"pairs"` → an ordered array of `{ "name": …, "value": … }` objects
/// with every cookie kept separately (the shape browser-automation drivers use).
pub fn to_value(cookies: &[Cookie], output: &str) -> Result<Value, String> {
    match output {
        "object" => {
            let mut map = Map::new();
            for c in cookies {
                merge_scalar(&mut map, &c.name, Value::String(c.value.clone()));
            }
            Ok(Value::Object(map))
        }
        "pairs" => {
            let arr = cookies
                .iter()
                .map(|c| {
                    let mut o = Map::new();
                    o.insert("name".into(), Value::String(c.name.clone()));
                    o.insert("value".into(), Value::String(c.value.clone()));
                    Value::Object(o)
                })
                .collect();
            Ok(Value::Array(arr))
        }
        other => Err(format!(
            "unknown output mode {other:?}: expected \"object\" or \"pairs\""
        )),
    }
}

/// Parse and return pretty JSON in the chosen output shape (chat / CLI / page).
pub fn run(input: &str, decode: bool, output: &str) -> Result<String, String> {
    let cookies = parse(input, decode);
    let value = to_value(&cookies, output)?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_basic_pairs_into_object() {
        let out = run("sessionid=abc123; theme=dark", true, "object").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, json!({ "sessionid": "abc123", "theme": "dark" }));
    }

    #[test]
    fn preserves_source_order_in_object() {
        // `preserve_order` keeps cookies in the order pasted, not alphabetical.
        let out = run("z=1; a=2; m=3", true, "object").unwrap();
        assert!(out.find("\"z\"") < out.find("\"a\""));
        assert!(out.find("\"a\"") < out.find("\"m\""));
    }

    #[test]
    fn percent_decodes_values_by_default() {
        let c = parse("path=%2Fhome%2Fuser; q=a%20b", true);
        assert_eq!(c[0].value, "/home/user");
        assert_eq!(c[1].value, "a b");
    }

    #[test]
    fn keeps_values_raw_when_decode_off() {
        let c = parse("path=%2Fhome", false);
        assert_eq!(c[0].value, "%2Fhome");
    }

    #[test]
    fn plus_stays_literal_not_a_space() {
        // Cookies are not form-urlencoded: `+` is a plus, not a space.
        let c = parse("q=a+b", true);
        assert_eq!(c[0].value, "a+b");
    }

    #[test]
    fn duplicate_names_collapse_to_array_in_object_mode() {
        let out = run("id=1; id=2; id=3", true, "object").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, json!({ "id": ["1", "2", "3"] }));
    }

    #[test]
    fn pairs_mode_keeps_every_cookie_in_order() {
        let out = run("id=1; theme=dark; id=2", true, "pairs").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v,
            json!([
                { "name": "id", "value": "1" },
                { "name": "theme", "value": "dark" },
                { "name": "id", "value": "2" }
            ])
        );
    }

    #[test]
    fn strips_leading_cookie_header_name() {
        let c = parse("Cookie: a=1; b=2", true);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].name, "a");
        assert_eq!(c[1].name, "b");
    }

    #[test]
    fn unwraps_double_quoted_value() {
        let c = parse("sid=\"abc 123\"", true);
        assert_eq!(c[0].value, "abc 123");
    }

    #[test]
    fn bare_token_becomes_empty_value() {
        let c = parse("flag; a=1", true);
        assert_eq!(c[0].name, "flag");
        assert_eq!(c[0].value, "");
        assert_eq!(c[1].name, "a");
    }

    #[test]
    fn empty_input_yields_empty_result() {
        assert_eq!(run("", true, "object").unwrap(), "{}");
        assert_eq!(run("   ", true, "pairs").unwrap(), "[]");
    }

    #[test]
    fn skips_empty_segments_and_trims_whitespace() {
        let c = parse("  a=1 ;; ; b=2 ", true);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].name, "a");
        assert_eq!(c[1].value, "2");
    }

    #[test]
    fn lenient_on_bad_percent_escape() {
        let c = parse("discount=100%off", true);
        assert_eq!(c[0].value, "100%off");
    }

    #[test]
    fn errors_on_unknown_output_mode() {
        let err = run("a=1", true, "yaml").unwrap_err();
        assert!(err.contains("unknown output mode"));
        assert!(err.contains("object"));
    }
}
