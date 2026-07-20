//! gizza-ai/regex-to-json core — parse each line of text with a named-capture
//! regular expression and emit structured JSON objects keyed by group name.
//! Pure-Rust (`regex` + `serde_json` with `preserve_order` so keys keep the
//! pattern's group order). No wafer/wasm-bindgen deps; shared by the chat skill
//! block, the CLI, and the web page.
//!
//! Distinct from `regex-extract` (a flat list of one group's matches) and
//! `regex-tester` (a per-match span/group debugging breakdown): this tool is a
//! text→data converter — every named capture group becomes a JSON key, every
//! line (or every match, with `all_matches`) becomes one JSON object.

use regex::RegexBuilder;
use serde_json::{Map, Value};

/// Maximum accepted input size in bytes (1 MB). The regex engine is
/// linear-time, but the browser page and chat sandbox share small memory
/// budgets — reject anything larger with an actionable error.
pub const MAX_TEXT_BYTES: usize = 1_000_000;

/// What to do with a non-blank line the pattern does not match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unmatched {
    Skip,
    Keep,
    Fail,
}

impl Unmatched {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "skip" => Ok(Self::Skip),
            "keep" => Ok(Self::Keep),
            "fail" => Ok(Self::Fail),
            other => Err(format!(
                "unknown unmatched mode '{other}' — use skip, keep, or fail"
            )),
        }
    }
}

/// Output shape for the emitted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Json,
    Compact,
    Ndjson,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "json" => Ok(Self::Json),
            "compact" => Ok(Self::Compact),
            "ndjson" => Ok(Self::Ndjson),
            other => Err(format!(
                "unknown output format '{other}' — use json, compact, or ndjson"
            )),
        }
    }
}

/// Coerce a captured string into a typed JSON value.
///
/// Rules (deliberately conservative so IDs are never mangled):
/// - exact `true` / `false` → boolean; exact `null` → null;
/// - plain integers (`42`, `-7`, `0`) → number, but values with leading zeros
///   (`007`) or that overflow i64 stay strings;
/// - plain decimals (`3.14`, `-0.5`) → number;
/// - everything else (scientific notation, hex, `1,000`, whitespace) → string.
fn coerce(s: &str) -> Value {
    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }
    let digits = s.strip_prefix('-').unwrap_or(s);
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    let plain_int = !int_part.is_empty()
        && int_part.bytes().all(|b| b.is_ascii_digit())
        && (int_part == "0" || !int_part.starts_with('0'));
    match frac_part {
        None if plain_int => match s.parse::<i64>() {
            Ok(n) => Value::from(n),
            Err(_) => Value::String(s.to_string()), // i64 overflow: keep exact
        },
        Some(f) if plain_int && !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
            match s.parse::<f64>() {
                Ok(n) if n.is_finite() => Value::from(n),
                _ => Value::String(s.to_string()),
            }
        }
        _ => Value::String(s.to_string()),
    }
}

/// Parse each non-blank line of `text` with `pattern` (which must contain at
/// least one named capture group) and render the resulting JSON records.
///
/// - `ignore_case`: case-insensitive matching.
/// - `all_matches`: emit one object per match instead of one per line (a line
///   with three matches yields three objects).
/// - `unmatched`: `skip` (drop non-matching lines), `keep` (emit
///   `{"_raw": "<line>"}`), or `fail` (error on the first non-matching line).
/// - `coerce_types`: convert number-, boolean-, and null-looking captures into
///   real JSON types (see [`coerce`]).
/// - `output`: `json` (pretty array), `compact` (one-line array), or `ndjson`
///   (one object per line).
///
/// Blank / whitespace-only lines are ignored entirely (they are neither
/// matched nor counted as unmatched); trailing `\r` is stripped so CRLF input
/// behaves like LF. A named group that did not participate in a match is
/// emitted as `null` so every record has the same keys, in pattern order.
pub fn to_json(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    all_matches: bool,
    unmatched: &str,
    coerce_types: bool,
    output: &str,
) -> Result<String, String> {
    let unmatched = Unmatched::parse(unmatched.trim())?;
    let output = Output::parse(output.trim())?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "text is too large ({} bytes) — the limit is {MAX_TEXT_BYTES} bytes (1 MB)",
            text.len()
        ));
    }
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }
    let re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    let names: Vec<&str> = re.capture_names().flatten().collect();
    if names.is_empty() {
        return Err(
            "the pattern has no named capture groups — wrap each field you want as a JSON key \
             in (?<name>…), e.g. (?<level>[A-Z]+) (?<message>.*)"
                .to_string(),
        );
    }

    let mut records: Vec<Value> = Vec::new();
    for (idx, raw_line) in text.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.trim().is_empty() {
            continue;
        }
        let mut matched = false;
        for caps in re.captures_iter(line) {
            matched = true;
            let mut obj = Map::with_capacity(names.len());
            for name in &names {
                let value = match caps.name(name) {
                    Some(m) if coerce_types => coerce(m.as_str()),
                    Some(m) => Value::String(m.as_str().to_string()),
                    None => Value::Null, // optional group that did not participate
                };
                obj.insert((*name).to_string(), value);
            }
            records.push(Value::Object(obj));
            if !all_matches {
                break;
            }
        }
        if !matched {
            match unmatched {
                Unmatched::Skip => {}
                Unmatched::Keep => {
                    let mut obj = Map::with_capacity(1);
                    obj.insert("_raw".to_string(), Value::String(line.to_string()));
                    records.push(Value::Object(obj));
                }
                Unmatched::Fail => {
                    return Err(format!(
                        "line {} does not match the pattern: {line}",
                        idx + 1
                    ));
                }
            }
        }
    }

    let rendered = match output {
        Output::Json => serde_json::to_string_pretty(&records),
        Output::Compact => serde_json::to_string(&records),
        Output::Ndjson => {
            return Ok(records
                .iter()
                .map(|r| serde_json::to_string(r).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n"));
        }
    };
    rendered.map_err(|e| format!("failed to serialize JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "2026-07-20 14:03:11 ERROR auth Failed login for alice\n\
                       2026-07-20 14:03:15 INFO http GET /health 200\n";
    const LOG_PATTERN: &str =
        r"(?<date>\d{4}-\d{2}-\d{2}) (?<time>\S+) (?<level>[A-Z]+) (?<module>\S+) (?<message>.*)";

    #[test]
    fn parses_lines_into_pretty_json_array() {
        let out = to_json(LOG, LOG_PATTERN, false, false, "skip", false, "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["level"], "ERROR");
        assert_eq!(v[1]["message"], "GET /health 200");
        // preserve_order: keys come out in pattern order.
        let keys: Vec<&str> = v[0].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["date", "time", "level", "module", "message"]);
        assert!(out.contains('\n'), "pretty output is multi-line");
    }

    #[test]
    fn python_style_named_groups_work() {
        let out = to_json(
            "a=1",
            r"(?P<key>\w+)=(?P<value>\w+)",
            false,
            false,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        assert_eq!(out, r#"[{"key":"a","value":"1"}]"#);
    }

    #[test]
    fn ndjson_emits_one_object_per_line() {
        let out = to_json(LOG, LOG_PATTERN, false, false, "skip", false, "ndjson").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        for l in &lines {
            let v: Value = serde_json::from_str(l).unwrap();
            assert!(v.is_object());
        }
        assert!(lines[0].starts_with(r#"{"date":"2026-07-20""#));
    }

    #[test]
    fn compact_is_single_line() {
        let out = to_json("x 1", r"(?<a>\w+) (?<b>\d+)", false, false, "skip", false, "compact")
            .unwrap();
        assert_eq!(out, r#"[{"a":"x","b":"1"}]"#);
    }

    #[test]
    fn all_matches_emits_object_per_match() {
        let out = to_json(
            "a=1 b=2 c=3",
            r"(?<key>\w+)=(?<value>\d+)",
            false,
            true,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[2]["key"], "c");
        // Without all_matches only the first match on the line counts.
        let first = to_json(
            "a=1 b=2 c=3",
            r"(?<key>\w+)=(?<value>\d+)",
            false,
            false,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn ignore_case_flag() {
        let out = to_json(
            "error: boom",
            r"(?<level>ERROR)",
            true,
            false,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        assert_eq!(out, r#"[{"level":"error"}]"#);
        let none = to_json(
            "error: boom",
            r"(?<level>ERROR)",
            false,
            false,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        assert_eq!(none, "[]");
    }

    #[test]
    fn unmatched_skip_keep_fail() {
        let text = "a=1\nnot a pair\nb=2";
        let pat = r"^(?<key>\w+)=(?<value>\d+)$";
        let skip = to_json(text, pat, false, false, "skip", false, "compact").unwrap();
        assert_eq!(serde_json::from_str::<Value>(&skip).unwrap().as_array().unwrap().len(), 2);

        let keep = to_json(text, pat, false, false, "keep", false, "compact").unwrap();
        let v: Value = serde_json::from_str(&keep).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[1]["_raw"], "not a pair");

        let err = to_json(text, pat, false, false, "fail", false, "compact").unwrap_err();
        assert!(err.contains("line 2"), "names the 1-based line: {err}");
        assert!(err.contains("not a pair"));
    }

    #[test]
    fn coerce_types_numbers_bools_null() {
        let out = to_json(
            "42 -3.14 true null 007 1e5",
            r"(?<int>\S+) (?<float>\S+) (?<bool>\S+) (?<null>\S+) (?<padded>\S+) (?<sci>\S+)",
            false,
            false,
            "skip",
            true,
            "compact",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"int":42,"float":-3.14,"bool":true,"null":null,"padded":"007","sci":"1e5"}]"#
        );
    }

    #[test]
    fn coercion_off_keeps_strings() {
        let out = to_json(
            "42 true",
            r"(?<n>\S+) (?<b>\S+)",
            false,
            false,
            "skip",
            false,
            "compact",
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":"42","b":"true"}]"#);
    }

    #[test]
    fn i64_overflow_stays_string() {
        let out = to_json(
            "99999999999999999999",
            r"(?<big>\d+)",
            false,
            false,
            "skip",
            true,
            "compact",
        )
        .unwrap();
        assert_eq!(out, r#"[{"big":"99999999999999999999"}]"#);
    }

    #[test]
    fn optional_group_becomes_null() {
        let out = to_json(
            "GET /a\nPOST /b 42",
            r"(?<method>[A-Z]+) (?<path>\S+)( (?<ms>\d+))?",
            false,
            false,
            "skip",
            true,
            "compact",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"method":"GET","path":"/a","ms":null},{"method":"POST","path":"/b","ms":42}]"#
        );
    }

    #[test]
    fn blank_lines_and_crlf_are_handled() {
        let out = to_json(
            "a=1\r\n\r\n   \nb=2\r\n",
            r"^(?<key>\w+)=(?<value>\d+)$",
            false,
            false,
            "fail", // blank lines must not trip `fail`
            false,
            "compact",
        )
        .unwrap();
        assert_eq!(out, r#"[{"key":"a","value":"1"},{"key":"b","value":"2"}]"#);
    }

    #[test]
    fn no_match_yields_empty_array() {
        let out = to_json("abc", r"(?<n>\d+)", false, false, "skip", false, "json").unwrap();
        assert_eq!(out, "[]");
        let nd = to_json("abc", r"(?<n>\d+)", false, false, "skip", false, "ndjson").unwrap();
        assert_eq!(nd, "");
    }

    #[test]
    fn pattern_without_named_groups_errors() {
        let err = to_json("a", r"(\d+)", false, false, "skip", false, "json").unwrap_err();
        assert!(err.contains("no named capture groups"), "{err}");
        assert!(err.contains("(?<name>"), "actionable hint: {err}");
    }

    #[test]
    fn invalid_pattern_errors() {
        let err = to_json("a", r"(?<x>", false, false, "skip", false, "json").unwrap_err();
        assert!(err.contains("invalid regular expression"), "{err}");
    }

    #[test]
    fn empty_pattern_errors() {
        assert!(to_json("a", "", false, false, "skip", false, "json").is_err());
    }

    #[test]
    fn unknown_enum_values_error() {
        assert!(to_json("a", r"(?<x>a)", false, false, "banana", false, "json").is_err());
        assert!(to_json("a", r"(?<x>a)", false, false, "skip", false, "yaml").is_err());
    }

    #[test]
    fn size_cap_exact_boundary() {
        // Exactly MAX_TEXT_BYTES is accepted; one byte over is rejected.
        let at = "a".repeat(MAX_TEXT_BYTES);
        assert!(to_json(&at, r"(?<x>a+)", false, false, "skip", false, "compact").is_ok());
        let over = "a".repeat(MAX_TEXT_BYTES + 1);
        let err = to_json(&over, r"(?<x>a+)", false, false, "skip", false, "compact").unwrap_err();
        assert!(err.contains("too large"), "{err}");
        assert!(err.contains("1 MB"), "{err}");
    }
}
