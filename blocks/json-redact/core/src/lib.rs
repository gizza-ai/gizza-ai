//! json-redact core — detect and mask secrets (tokens, API keys, passwords,
//! emails, private keys) inside a JSON document while preserving its structure.
//! Pure-Rust (`serde_json` with `preserve_order`, `regex`); shared by the chat
//! skill block and the web page.
//!
//! Two detection modes run together:
//!   * **key-name** — a value is redacted when its object key looks sensitive
//!     (normalized lowercase, non-alphanumerics stripped, then substring/exact
//!     matched against a marker set, plus caller-supplied `extra_keys`).
//!   * **value pattern** — when `detect_values` is on, any *string* value that
//!     looks like a secret (JWT, AWS/OpenAI/GitHub/Stripe/Google/Slack keys, a
//!     PEM private-key block, or an email address) is redacted even if its key
//!     is innocuous.
//!
//! Structure and key order are preserved; only detected values are replaced with
//! the chosen [`Style`]. The result reports how many values were redacted and the
//! JSON path of each.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};

/// How a detected value is replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Replace the value with the caller's placeholder string (default `[REDACTED]`).
    Redacted,
    /// Replace the value with a fixed `***`.
    Mask,
    /// Replace the value with JSON `null`.
    Null,
    /// Replace the value with an empty string `""`.
    Empty,
    /// Replace a string with `*` repeated to its original character length
    /// (non-string values fall back to `***`).
    PreserveLength,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "redacted" | "" => Ok(Style::Redacted),
            "mask" => Ok(Style::Mask),
            "null" => Ok(Style::Null),
            "empty" => Ok(Style::Empty),
            "preserve-length" | "preserve_length" => Ok(Style::PreserveLength),
            other => Err(format!(
                "unknown style '{other}' (use 'redacted', 'mask', 'null', 'empty' or 'preserve-length')"
            )),
        }
    }
}

/// Outcome of a redaction pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedactResult {
    /// The redacted document, pretty-printed JSON.
    pub redacted: String,
    /// Number of values that were redacted.
    pub count: usize,
    /// JSON path of each redacted value, in document order (e.g. `$.user.api_key`,
    /// `$.tokens[0]`).
    pub paths: Vec<String>,
}

/// Options controlling a redaction pass.
pub struct Options<'a> {
    pub style: Style,
    /// Replacement text for [`Style::Redacted`].
    pub placeholder: &'a str,
    /// Also scan string *values* for secret patterns, not just key names.
    pub detect_values: bool,
    /// Extra sensitive key markers (already normalized) supplied by the caller.
    pub extra_keys: Vec<String>,
}

/// Substring markers: a value is redacted when its normalized key *contains* one.
const KEY_SUBSTRINGS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "apikey",
    "token",
    "privatekey",
    "accesskey",
    "clientsecret",
    "credential",
    "authorization",
    "sessionid",
    "sessiontoken",
    "encryptionkey",
    "signingkey",
    "connectionstring",
    "email",
    "creditcard",
    "cardnumber",
];

/// Exact markers: a value is redacted when its normalized key *equals* one.
const KEY_EXACT: &[&str] = &["pwd", "pass", "pin", "cvv", "ssn", "auth", "bearer", "otp", "cookie"];

/// Lowercase and drop every non-alphanumeric char, so `X-API_Key` → `xapikey`.
pub fn normalize_key(k: &str) -> String {
    k.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn key_is_secret(key: &str, extra: &[String]) -> bool {
    let n = normalize_key(key);
    if n.is_empty() {
        return false;
    }
    if KEY_EXACT.contains(&n.as_str()) {
        return true;
    }
    if KEY_SUBSTRINGS.iter().any(|m| n.contains(m)) {
        return true;
    }
    extra.iter().any(|m| !m.is_empty() && n.contains(m.as_str()))
}

struct ValuePatterns {
    email: Regex,
    jwt: Regex,
    aws: Regex,
    openai: Regex,
    github: Regex,
    stripe: Regex,
    google: Regex,
    slack: Regex,
}

fn value_patterns() -> &'static ValuePatterns {
    static P: OnceLock<ValuePatterns> = OnceLock::new();
    P.get_or_init(|| ValuePatterns {
        // Whole-value email address.
        email: Regex::new(r"^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$").unwrap(),
        // JWT: three base64url segments, header starting `eyJ`.
        jwt: Regex::new(r"eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+").unwrap(),
        // AWS access-key id.
        aws: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
        // OpenAI-style secret key (incl. sk-proj-).
        openai: Regex::new(r"sk-[A-Za-z0-9_\-]{20,}").unwrap(),
        // GitHub PAT / OAuth / refresh tokens (ghp_, gho_, ghu_, ghs_, ghr_).
        github: Regex::new(r"gh[pousr]_[A-Za-z0-9]{20,}").unwrap(),
        // Stripe live/test keys.
        stripe: Regex::new(r"[srp]k_(live|test)_[A-Za-z0-9]{10,}").unwrap(),
        // Google API key.
        google: Regex::new(r"AIza[0-9A-Za-z_\-]{35}").unwrap(),
        // Slack tokens.
        slack: Regex::new(r"xox[baprs]-[A-Za-z0-9\-]{10,}").unwrap(),
    })
}

/// True if `s` looks like a secret worth redacting.
pub fn value_is_secret(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains("PRIVATE KEY-----") {
        return true;
    }
    let p = value_patterns();
    p.email.is_match(t)
        || p.jwt.is_match(t)
        || p.aws.is_match(t)
        || p.openai.is_match(t)
        || p.github.is_match(t)
        || p.stripe.is_match(t)
        || p.google.is_match(t)
        || p.slack.is_match(t)
}

/// Build the replacement value for a detected `original`, per `style`.
fn redaction_value(original: &Value, opts: &Options) -> Value {
    match opts.style {
        Style::Null => Value::Null,
        Style::Empty => Value::String(String::new()),
        Style::Redacted => Value::String(opts.placeholder.to_string()),
        Style::Mask => Value::String("***".to_string()),
        Style::PreserveLength => {
            let n = match original {
                Value::String(s) => s.chars().count(),
                other => other.to_string().chars().count(),
            };
            Value::String("*".repeat(n))
        }
    }
}

/// Append a child path segment for `key` onto `parent` (`$.id` or `$["odd key"]`).
fn child_key_path(parent: &str, key: &str) -> String {
    let simple = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !key.chars().next().unwrap().is_ascii_digit();
    if simple {
        format!("{parent}.{key}")
    } else {
        format!("{parent}[{}]", serde_json::to_string(key).unwrap())
    }
}

fn walk(v: &Value, path: &str, opts: &Options, out: &mut Vec<String>) -> Value {
    match v {
        Value::Object(map) => {
            let mut new_map = Map::new();
            for (k, val) in map {
                let child = child_key_path(path, k);
                if key_is_secret(k, &opts.extra_keys) {
                    out.push(child);
                    new_map.insert(k.clone(), redaction_value(val, opts));
                } else {
                    new_map.insert(k.clone(), walk(val, &child, opts, out));
                }
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .enumerate()
                .map(|(i, val)| walk(val, &format!("{path}[{i}]"), opts, out))
                .collect(),
        ),
        Value::String(s) => {
            if opts.detect_values && value_is_secret(s) {
                out.push(path.to_string());
                redaction_value(v, opts)
            } else {
                v.clone()
            }
        }
        other => other.clone(),
    }
}

/// Parse `input` as JSON, redact detected secrets, and return the redacted
/// document (pretty JSON) plus a report. Returns `Err` on invalid JSON.
pub fn redact_json(input: &str, opts: &Options) -> Result<RedactResult, String> {
    let parsed: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut paths = Vec::new();
    let redacted_value = walk(&parsed, "$", opts, &mut paths);
    let redacted = serde_json::to_string_pretty(&redacted_value)
        .map_err(|e| format!("failed to serialize output: {e}"))?;
    Ok(RedactResult {
        count: paths.len(),
        paths,
        redacted,
    })
}

/// Split a comma-separated `extra_keys` argument into normalized markers.
pub fn parse_extra_keys(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(normalize_key)
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(style: Style) -> Options<'static> {
        Options {
            style,
            placeholder: "[REDACTED]",
            detect_values: true,
            extra_keys: Vec::new(),
        }
    }

    #[test]
    fn redacts_by_key_name() {
        let input = r#"{"user":"ada","password":"hunter2","api_key":"abc123"}"#;
        let r = redact_json(input, &opts(Style::Redacted)).unwrap();
        assert_eq!(r.count, 2);
        assert_eq!(r.paths, vec!["$.password", "$.api_key"]);
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["user"], "ada");
        assert_eq!(v["password"], "[REDACTED]");
        assert_eq!(v["api_key"], "[REDACTED]");
    }

    #[test]
    fn normalized_key_variants_match() {
        // X-API-Key, clientSecret, DB_PASSWORD all normalize to markers.
        let input = r#"{"X-API-Key":"k","clientSecret":"s","DB_PASSWORD":"p","name":"ok"}"#;
        let r = redact_json(input, &opts(Style::Mask)).unwrap();
        assert_eq!(r.count, 3);
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["X-API-Key"], "***");
        assert_eq!(v["clientSecret"], "***");
        assert_eq!(v["DB_PASSWORD"], "***");
        assert_eq!(v["name"], "ok");
    }

    #[test]
    fn detects_secret_values_under_innocuous_keys() {
        // The key "note" isn't sensitive, but the value is a JWT / AWS key / email.
        let input = r#"{"note":"AKIAIOSFODNN7EXAMPLE","contact":"ada@example.com","plain":"hello"}"#;
        let r = redact_json(input, &opts(Style::Redacted)).unwrap();
        assert_eq!(r.count, 2);
        assert_eq!(r.paths, vec!["$.note", "$.contact"]);
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["plain"], "hello");
    }

    #[test]
    fn detect_values_off_leaves_innocuous_keys() {
        let mut o = opts(Style::Redacted);
        o.detect_values = false;
        let input = r#"{"note":"AKIAIOSFODNN7EXAMPLE"}"#;
        let r = redact_json(input, &o).unwrap();
        assert_eq!(r.count, 0);
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["note"], "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn recurses_nested_objects_and_arrays() {
        let input =
            r#"{"a":{"b":{"token":"t"}},"list":[{"secret":"x"},{"ok":1}],"items":["ghp_012345678901234567890123"]}"#;
        let r = redact_json(input, &opts(Style::Redacted)).unwrap();
        // token by key (nested), secret by key (in array), and a GitHub token
        // detected by VALUE under the innocuous "items" array.
        assert_eq!(
            r.paths,
            vec!["$.a.b.token", "$.list[0].secret", "$.items[0]"]
        );
        assert_eq!(r.count, 3);
    }

    #[test]
    fn preserve_length_style() {
        let input = r#"{"password":"hunter2"}"#;
        let r = redact_json(input, &opts(Style::PreserveLength)).unwrap();
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["password"], "*******"); // 7 chars
    }

    #[test]
    fn null_and_empty_styles() {
        let input = r#"{"password":"hunter2","secret":"x"}"#;
        let rn = redact_json(input, &opts(Style::Null)).unwrap();
        let vn: Value = serde_json::from_str(&rn.redacted).unwrap();
        assert!(vn["password"].is_null());
        let re = redact_json(input, &opts(Style::Empty)).unwrap();
        let ve: Value = serde_json::from_str(&re.redacted).unwrap();
        assert_eq!(ve["secret"], "");
    }

    #[test]
    fn extra_keys_add_markers() {
        let mut o = opts(Style::Mask);
        o.extra_keys = parse_extra_keys("nickname, phone");
        let input = r#"{"nickname":"ace","phone":"555","name":"ok"}"#;
        let r = redact_json(input, &o).unwrap();
        assert_eq!(r.count, 2);
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["name"], "ok");
        assert_eq!(v["nickname"], "***");
    }

    #[test]
    fn preserves_key_order() {
        let input = r#"{"z":1,"password":"p","a":2}"#;
        let r = redact_json(input, &opts(Style::Mask)).unwrap();
        // preserve_order keeps z, password, a in that order.
        assert!(r.redacted.find("\"z\"").unwrap() < r.redacted.find("\"password\"").unwrap());
        assert!(r.redacted.find("\"password\"").unwrap() < r.redacted.find("\"a\"").unwrap());
    }

    #[test]
    fn non_string_secret_value_redacted_by_key() {
        // A numeric password value is still redacted by its key.
        let input = r#"{"pin":1234}"#;
        let r = redact_json(input, &opts(Style::PreserveLength)).unwrap();
        let v: Value = serde_json::from_str(&r.redacted).unwrap();
        assert_eq!(v["pin"], "****"); // "1234".len() == 4
    }

    #[test]
    fn invalid_json_errors() {
        let err = redact_json("{not json}", &opts(Style::Redacted)).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "got: {err}");
    }

    #[test]
    fn unknown_style_errors() {
        assert!(Style::parse("shred").is_err());
        assert_eq!(Style::parse("preserve_length").unwrap(), Style::PreserveLength);
    }
}
