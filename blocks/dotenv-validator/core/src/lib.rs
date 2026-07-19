//! dotenv-validator core — a pure linter for `.env` (dotenv) files. Shared by
//! the chat skill block and the web page; no wafer/wasm-bindgen deps.
//!
//! It parses a `.env` document line by line and reports lint issues: duplicate
//! keys, unquoted values containing spaces, malformed `${...}` interpolation,
//! and references to variables that are never defined in the file — plus the
//! usual syntax nits (missing `=`, invalid/lowercase key names, whitespace
//! around `=`, empty values, unterminated quotes, CRLF line endings). It never
//! rewrites the file; it only diagnoses.

use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone)]
struct Issue {
    /// 1-indexed source line, or 0 for a file-level issue (e.g. CRLF).
    line: usize,
    severity: Severity,
    rule: &'static str,
    message: String,
}

/// Lint the `.env` document in `env`.
///
/// - `allow_undefined`: comma-separated variable names that are supplied
///   externally (from the shell/CI) and must NOT be flagged as missing
///   interpolation references (e.g. `HOME,PATH,CI`). Matched case-sensitively.
/// - `check_interpolation`: when true, `${VAR}`/`$VAR` references are validated
///   (unclosed `${`, empty `${}`, and references to undefined variables).
/// - `require_quotes_for_spaces`: when true, an unquoted value that contains a
///   space is flagged (most loaders keep only the first word).
/// - `output`: `"report"` (default) a human-readable list, or `"json"` a
///   structured `{ok, keys, error_count, warning_count, issues[]}` object.
pub fn validate(
    env: &str,
    allow_undefined: &str,
    check_interpolation: bool,
    require_quotes_for_spaces: bool,
    output: &str,
) -> Result<String, String> {
    let output = {
        let o = output.trim();
        if o.is_empty() {
            "report"
        } else {
            o
        }
    };
    if output != "report" && output != "json" {
        return Err(format!(
            "output must be 'report' or 'json', got '{output}'"
        ));
    }

    let allow: HashSet<&str> = allow_undefined
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    // Pre-scan: collect every key defined anywhere in the file so an
    // interpolation reference resolves regardless of definition order.
    let mut defined: HashSet<String> = HashSet::new();
    for raw in env.split('\n') {
        if let Some((key, _)) = split_assignment(raw) {
            if !key.is_empty() {
                defined.insert(key.to_string());
            }
        }
    }

    let mut issues: Vec<Issue> = Vec::new();

    if env.contains("\r\n") {
        issues.push(Issue {
            line: 0,
            severity: Severity::Warning,
            rule: "crlf-line-ending",
            message: "file uses CRLF (\\r\\n) line endings; some dotenv loaders keep the trailing \\r as part of the value — save with LF".into(),
        });
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut key_count = 0usize;

    for (i, raw) in env.split('\n').enumerate() {
        let lineno = i + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let eq = match body.find('=') {
            Some(e) => e,
            None => {
                issues.push(Issue {
                    line: lineno,
                    severity: Severity::Error,
                    rule: "missing-equals",
                    message: format!(
                        "line is not a comment and has no '=' assignment: '{}'",
                        body.trim()
                    ),
                });
                continue;
            }
        };
        let raw_key = &body[..eq];
        let raw_val = &body[eq + 1..];
        let key = raw_key.trim();

        if raw_key.ends_with(|c: char| c.is_whitespace())
            || raw_val.starts_with(|c: char| c.is_whitespace())
        {
            issues.push(Issue {
                line: lineno,
                severity: Severity::Warning,
                rule: "space-around-equals",
                message: format!(
                    "'{key}' has whitespace around '='; write KEY=value with no spaces (some loaders keep the spaces)"
                ),
            });
        }

        if key.is_empty() {
            issues.push(Issue {
                line: lineno,
                severity: Severity::Error,
                rule: "empty-key",
                message: "assignment has an empty key before '='".into(),
            });
        } else {
            if !is_valid_key(key) {
                issues.push(Issue {
                    line: lineno,
                    severity: Severity::Error,
                    rule: "invalid-key-name",
                    message: format!(
                        "key '{key}' should match [A-Za-z_][A-Za-z0-9_]* — use letters, digits and '_' (no '-', spaces or leading digit)"
                    ),
                });
            } else if key.chars().any(|c| c.is_ascii_lowercase()) {
                issues.push(Issue {
                    line: lineno,
                    severity: Severity::Warning,
                    rule: "lowercase-key",
                    message: format!(
                        "key '{key}' is not UPPER_SNAKE_CASE; the convention is uppercase, e.g. '{}'",
                        key.to_ascii_uppercase()
                    ),
                });
            }
            if let Some(&first) = seen.get(key) {
                issues.push(Issue {
                    line: lineno,
                    severity: Severity::Warning,
                    rule: "duplicate-key",
                    message: format!(
                        "key '{key}' was already defined on line {first}; most loaders keep only the last value"
                    ),
                });
            } else {
                // Count each distinct key once (a duplicate re-assignment is
                // reported separately and does not add to the key tally).
                key_count += 1;
                seen.insert(key.to_string(), lineno);
            }
        }

        analyze_value(
            raw_val,
            key,
            lineno,
            &defined,
            &allow,
            check_interpolation,
            require_quotes_for_spaces,
            &mut issues,
        );
    }

    issues.sort_by_key(|it| it.line);

    if output == "json" {
        Ok(render_json(&issues, key_count))
    } else {
        Ok(render_report(&issues, key_count))
    }
}

/// Split a raw line into `(key, value)` if it is a `KEY=VALUE` assignment
/// (handling an `export ` prefix), skipping blank lines and comments. Returns
/// the trimmed key and the raw value.
fn split_assignment(raw: &str) -> Option<(&str, &str)> {
    let line = raw.strip_suffix('\r').unwrap_or(raw);
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let eq = body.find('=')?;
    Some((body[..eq].trim(), &body[eq + 1..]))
}

/// A dotenv key is `[A-Za-z_][A-Za-z0-9_]*` (letters, digits, underscore; no
/// leading digit).
fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[allow(clippy::too_many_arguments)]
fn analyze_value(
    raw_val: &str,
    key: &str,
    lineno: usize,
    defined: &HashSet<String>,
    allow: &HashSet<&str>,
    check_interpolation: bool,
    require_quotes_for_spaces: bool,
    issues: &mut Vec<Issue>,
) {
    let v = raw_val.trim_start();
    if v.is_empty() {
        issues.push(Issue {
            line: lineno,
            severity: Severity::Warning,
            rule: "empty-value",
            message: format!("key '{key}' has no value"),
        });
        return;
    }

    let first = v.as_bytes()[0];
    if first == b'"' || first == b'\'' {
        let quote = first as char;
        let interp = quote == '"'; // single quotes are literal — no interpolation
        match find_closing_quote(&v[1..], quote) {
            Some(rel) => {
                let inner = &v[1..1 + rel];
                if check_interpolation && interp {
                    scan_interpolation(inner, key, lineno, defined, allow, issues);
                }
            }
            None => {
                issues.push(Issue {
                    line: lineno,
                    severity: Severity::Error,
                    rule: "unterminated-quote",
                    message: format!(
                        "key '{key}' has an unterminated {} quote",
                        if quote == '"' { "double" } else { "single" }
                    ),
                });
            }
        }
        return;
    }

    // Unquoted value: an inline ` #comment` ends it.
    let (value_part, _had_comment) = split_inline_comment(v);
    let effective = value_part.trim_end();

    if effective != value_part {
        issues.push(Issue {
            line: lineno,
            severity: Severity::Warning,
            rule: "trailing-whitespace",
            message: format!(
                "value for '{key}' has trailing whitespace that most loaders keep"
            ),
        });
    }

    if require_quotes_for_spaces && effective.contains(char::is_whitespace) {
        issues.push(Issue {
            line: lineno,
            severity: Severity::Warning,
            rule: "unquoted-space",
            message: format!(
                "value for '{key}' contains a space but is not quoted; most loaders keep only the first word — wrap it in quotes"
            ),
        });
    }

    if check_interpolation {
        scan_interpolation(effective, key, lineno, defined, allow, issues);
    }
}

/// Byte offset of the first unescaped closing `quote` in `s` (which does not
/// include the opening quote). Backslash escapes are honored only for double
/// quotes; single-quoted dotenv values are literal.
fn find_closing_quote(s: &str, quote: char) -> Option<usize> {
    let bytes = s.as_bytes();
    let q = quote as u8;
    let escapes = quote == '"';
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if escapes && b == b'\\' {
            i += 2;
            continue;
        }
        if b == q {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Split an unquoted value at its inline comment (` #...`): a `#` preceded by
/// whitespace starts a comment. Returns `(value_before_comment, had_comment)`.
fn split_inline_comment(v: &str) -> (&str, bool) {
    let bytes = v.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#' && bytes[i - 1].is_ascii_whitespace() {
            return (&v[..i], true);
        }
    }
    (v, false)
}

/// Scan `inner` for `${VAR}` / `$VAR` interpolation, flagging malformed
/// interpolation and references to variables not defined in the file.
fn scan_interpolation(
    inner: &str,
    key: &str,
    lineno: usize,
    defined: &HashSet<String>,
    allow: &HashSet<&str>,
    issues: &mut Vec<Issue>,
) {
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        // A backslash-escaped `\$` is a literal dollar, not interpolation.
        if i > 0 && bytes[i - 1] == b'\\' {
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            match inner[i + 2..].find('}') {
                Some(rel) => {
                    let expr = &inner[i + 2..i + 2 + rel];
                    let name = leading_identifier(expr);
                    if name.is_empty() {
                        issues.push(Issue {
                            line: lineno,
                            severity: Severity::Error,
                            rule: "bad-interpolation",
                            message: format!(
                                "value for '{key}' has an empty '${{}}' interpolation"
                            ),
                        });
                    } else {
                        check_reference(name, key, lineno, defined, allow, issues);
                    }
                    i = i + 2 + rel + 1;
                    continue;
                }
                None => {
                    issues.push(Issue {
                        line: lineno,
                        severity: Severity::Error,
                        rule: "bad-interpolation",
                        message: format!(
                            "value for '{key}' has an unclosed '${{' interpolation (missing '}}')"
                        ),
                    });
                    break;
                }
            }
        } else if i + 1 < bytes.len()
            && (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_')
        {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let name = &inner[i + 1..j];
            check_reference(name, key, lineno, defined, allow, issues);
            i = j;
            continue;
        }
        // A bare `$` (e.g. a `$5` price) is a literal — no interpolation.
        i += 1;
    }
}

/// Leading `[A-Za-z_][A-Za-z0-9_]*` of an interpolation expression, so
/// `${VAR:-default}` and `${VAR}` both yield `VAR`.
fn leading_identifier(expr: &str) -> &str {
    let bytes = expr.as_bytes();
    if bytes.is_empty() || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return "";
    }
    let mut j = 1;
    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
    }
    &expr[..j]
}

fn check_reference(
    name: &str,
    key: &str,
    lineno: usize,
    defined: &HashSet<String>,
    allow: &HashSet<&str>,
    issues: &mut Vec<Issue>,
) {
    if !defined.contains(name) && !allow.contains(name) {
        issues.push(Issue {
            line: lineno,
            severity: Severity::Warning,
            rule: "undefined-reference",
            message: format!(
                "value for '{key}' references ${{{name}}} which is not defined in this file; add it, or list it in allow_undefined if it comes from the shell/CI"
            ),
        });
    }
}

fn render_report(issues: &[Issue], key_count: usize) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;

    if issues.is_empty() {
        return format!(
            ".env validation: no issues found — {key_count} {} look valid.",
            plural(key_count, "key", "keys")
        );
    }

    let mut out = format!(
        ".env validation: {} {} ({} {}, {} {}) across {} {}\n",
        issues.len(),
        plural(issues.len(), "issue", "issues"),
        errors,
        plural(errors, "error", "errors"),
        warnings,
        plural(warnings, "warning", "warnings"),
        key_count,
        plural(key_count, "key", "keys"),
    );
    for it in issues {
        let where_ = if it.line == 0 {
            "file".to_string()
        } else {
            format!("line {}", it.line)
        };
        out.push_str(&format!(
            "  {:<8}  {:<7}  {:<20}  {}\n",
            where_,
            it.severity.as_str(),
            it.rule,
            it.message
        ));
    }
    // Trim the trailing newline for a clean single-value result.
    out.pop();
    out
}

fn render_json(issues: &[Issue], key_count: usize) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;
    let arr: Vec<serde_json::Value> = issues
        .iter()
        .map(|it| {
            serde_json::json!({
                "line": if it.line == 0 { serde_json::Value::Null } else { serde_json::json!(it.line) },
                "severity": it.severity.as_str(),
                "rule": it.rule,
                "message": it.message,
            })
        })
        .collect();
    let val = serde_json::json!({
        "ok": errors == 0,
        "keys": key_count,
        "error_count": errors,
        "warning_count": warnings,
        "issues": arr,
    });
    serde_json::to_string_pretty(&val).unwrap_or_else(|_| "{}".into())
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(env: &str) -> String {
        validate(env, "", true, true, "report").unwrap()
    }

    #[test]
    fn clean_file_has_no_issues() {
        let env = "# comment\nDATABASE_URL=postgres://localhost/app\nAPI_KEY=abc123\nGREETING=\"hello world\"\n";
        let out = report(env);
        assert!(out.contains("no issues found"), "got: {out}");
        assert!(out.contains("3 keys"), "got: {out}");
    }

    #[test]
    fn flags_duplicate_key() {
        let out = report("FOO=1\nFOO=2\n");
        assert!(out.contains("duplicate-key"), "got: {out}");
        assert!(out.contains("already defined on line 1"), "got: {out}");
    }

    #[test]
    fn flags_unquoted_space() {
        let out = report("GREETING=hello world\n");
        assert!(out.contains("unquoted-space"), "got: {out}");
        // A quoted equivalent must NOT be flagged.
        let ok = report("GREETING=\"hello world\"\n");
        assert!(!ok.contains("unquoted-space"), "got: {ok}");
    }

    #[test]
    fn flags_undefined_reference_and_resolves_defined_ones() {
        let out = report("HOST=db.example.com\nURL=http://${HOST}/${MISSING}\n");
        assert!(out.contains("undefined-reference"), "got: {out}");
        assert!(out.contains("${MISSING}"), "got: {out}");
        // ${HOST} is defined earlier, so it must not be flagged.
        assert!(!out.contains("${HOST}"), "got: {out}");
    }

    #[test]
    fn allow_undefined_suppresses_reference() {
        let out = validate("URL=http://${HOST}/x\n", "HOST", true, true, "report").unwrap();
        assert!(out.contains("no issues found"), "got: {out}");
    }

    #[test]
    fn single_quotes_are_literal() {
        // Single-quoted values do not interpolate, so ${MISSING} is not a ref.
        let out = report("X='literal ${MISSING}'\n");
        assert!(!out.contains("undefined-reference"), "got: {out}");
    }

    #[test]
    fn flags_bad_interpolation() {
        let out = report("URL=http://${HOST/path\n");
        assert!(out.contains("bad-interpolation"), "got: {out}");
        assert!(out.contains("unclosed"), "got: {out}");
    }

    #[test]
    fn flags_invalid_and_lowercase_keys() {
        let out = report("API-KEY=1\n");
        assert!(out.contains("invalid-key-name"), "got: {out}");
        let out2 = report("api_key=1\n");
        assert!(out2.contains("lowercase-key"), "got: {out2}");
    }

    #[test]
    fn flags_missing_equals_and_empty_value() {
        let out = report("JUST_A_WORD\n");
        assert!(out.contains("missing-equals"), "got: {out}");
        let out2 = report("EMPTY=\n");
        assert!(out2.contains("empty-value"), "got: {out2}");
    }

    #[test]
    fn json_output_shape() {
        let out = validate("FOO=1\nFOO=2\n", "", true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], serde_json::json!(true)); // duplicate is a warning, not an error
        assert_eq!(v["keys"], serde_json::json!(1));
        assert_eq!(v["warning_count"], serde_json::json!(1));
        assert_eq!(v["issues"][0]["rule"], serde_json::json!("duplicate-key"));
        assert_eq!(v["issues"][0]["line"], serde_json::json!(2));
    }

    #[test]
    fn invalid_output_is_an_error() {
        let err = validate("FOO=1\n", "", true, true, "xml").unwrap_err();
        assert!(err.contains("output must be"), "got: {err}");
    }

    #[test]
    fn errors_make_ok_false_in_json() {
        let out = validate("API-KEY=1\n", "", true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], serde_json::json!(false));
        assert_eq!(v["error_count"], serde_json::json!(1));
    }
}
