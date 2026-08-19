//! env-schema-validate core — validates a `.env` document against a DECLARED
//! schema of required keys, value types and allowed values. Shared by the chat
//! skill block and the web page; no wafer/wasm-bindgen deps.
//!
//! Two schema dialects are understood:
//!
//! * **rules** — one `KEY=rule|rule:arg` line per variable, e.g.
//!   `PORT=required|port`, `NODE_ENV=required|enum:dev,staging,prod`,
//!   `API_KEY=min:32`. A key with no rules at all is simply required.
//! * **json-schema** — a JSON Schema object (`type`/`required`/`properties`
//!   with `enum`, `minimum`/`maximum`, `minLength`/`maxLength`, `pattern`,
//!   `format`, `default`).
//!
//! A third input shape, **example**, treats the schema text as a `.env.example`
//! template: every key it declares is required, values are ignored.
//!
//! Nothing is rewritten — the tool only diagnoses, reporting each problem with
//! the `.env` line number, a severity and the rule that produced it.
//!
//! Besides the `report` and `json` outputs there are two more: `markdown`
//! (the same findings as tables, plus a documentation table of every declared
//! variable) and `schema`, which ignores the schema input and writes a starter
//! rules schema inferred from the `.env` itself.

use std::collections::{HashMap, HashSet};

use regex::Regex;

/// Every rule token the `rules` dialect accepts, for error messages.
const SUPPORTED_RULES: &str = "required, optional, string, number, integer, boolean, port, url, email, host, json, enum:a,b, min:N, max:N, pattern:REGEX, secure, default:VALUE";

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
    /// 1-indexed `.env` line, or 0 for a file-level issue (a missing key has no line).
    line: usize,
    key: String,
    severity: Severity,
    rule: &'static str,
    message: String,
}

/// The value type a declared key must hold.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ty {
    Str,
    Number,
    Integer,
    Boolean,
    Port,
    Url,
    Email,
    Host,
    Json,
}

impl Ty {
    fn as_str(self) -> &'static str {
        match self {
            Ty::Str => "string",
            Ty::Number => "number",
            Ty::Integer => "integer",
            Ty::Boolean => "boolean",
            Ty::Port => "port",
            Ty::Url => "url",
            Ty::Email => "email",
            Ty::Host => "host",
            Ty::Json => "json",
        }
    }

    fn parse(name: &str) -> Option<Ty> {
        Some(match name {
            "string" | "str" => Ty::Str,
            "number" | "num" | "float" => Ty::Number,
            "integer" | "int" => Ty::Integer,
            "boolean" | "bool" => Ty::Boolean,
            "port" => Ty::Port,
            "url" | "uri" => Ty::Url,
            "email" => Ty::Email,
            "host" | "hostname" => Ty::Host,
            "json" => Ty::Json,
            _ => return None,
        })
    }
}

/// One declared variable from the schema.
struct Decl {
    key: String,
    required: bool,
    ty: Ty,
    allowed: Option<Vec<String>>,
    min: Option<f64>,
    max: Option<f64>,
    pattern: Option<Regex>,
    secure: bool,
    default: Option<String>,
    /// Free-text note: a trailing `# comment` on a rules line, or a JSON Schema
    /// `description`. Documentation only — it is echoed in the markdown output.
    desc: Option<String>,
}

impl Decl {
    fn new(key: &str) -> Decl {
        Decl {
            key: key.to_string(),
            required: false,
            ty: Ty::Str,
            allowed: None,
            min: None,
            max: None,
            pattern: None,
            secure: false,
            default: None,
            desc: None,
        }
    }

    /// Everything beyond required/type, as a human list for the docs table.
    fn constraints(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(allowed) = &self.allowed {
            parts.push(format!("one of {}", allowed.join(", ")));
        }
        let bound = if matches!(self.ty, Ty::Number | Ty::Integer | Ty::Port) {
            "value"
        } else {
            "length"
        };
        match (self.min, self.max) {
            (Some(a), Some(b)) => parts.push(format!("{bound} {}–{}", trim_num(a), trim_num(b))),
            (Some(a), None) => parts.push(format!("{bound} ≥ {}", trim_num(a))),
            (None, Some(b)) => parts.push(format!("{bound} ≤ {}", trim_num(b))),
            (None, None) => {}
        }
        if let Some(p) = &self.pattern {
            parts.push(format!("matches /{}/", p.as_str()));
        }
        if self.secure {
            parts.push("strong secret".to_string());
        }
        if let Some(d) = &self.default {
            parts.push(format!("default {d}"));
        }
        parts.join("; ")
    }
}

/// One `KEY=value` pair parsed out of the `.env` document.
struct EnvEntry {
    line: usize,
    value: String,
}

/// Validate the `.env` document in `env` against `schema`.
///
/// - `schema_format`: `auto` (default — a schema starting with `{` is JSON
///   Schema, anything else is the rules dialect), `rules`, `json-schema` or
///   `example` (treat the schema text as a `.env.example`: every key required).
/// - `unknown_keys`: what to do with keys present in the `.env` but absent from
///   the schema — `warn` (default), `ignore` or `error`.
/// - `empty_is_missing`: when true (default) `KEY=` counts as not set, so a
///   required key with an empty value is reported missing rather than
///   type-checked.
/// - `output`: `report` (default, human-readable), `json` (structured, for CI),
///   `markdown` (findings + a documentation table, for a PR comment or job
///   summary) or `schema` (ignore the schema input and generate a starter rules
///   schema from the `.env` itself).
pub fn validate(
    env: &str,
    schema: &str,
    schema_format: &str,
    unknown_keys: &str,
    empty_is_missing: bool,
    output: &str,
) -> Result<String, String> {
    let output = non_empty_or(output, "report");
    if !matches!(output, "report" | "json" | "markdown" | "schema") {
        return Err(format!(
            "output must be 'report', 'json', 'markdown' or 'schema' (got '{output}')"
        ));
    }
    // `schema` generates a schema FROM the .env, so it needs no schema input
    // and runs before every schema-shaped validation below.
    if output == "schema" {
        return generate_schema(env);
    }
    let unknown_keys = non_empty_or(unknown_keys, "warn");
    if !matches!(unknown_keys, "warn" | "ignore" | "error") {
        return Err(format!(
            "unknown_keys must be 'warn', 'ignore' or 'error' (got '{unknown_keys}')"
        ));
    }
    let schema_format = non_empty_or(schema_format, "auto");
    if !matches!(schema_format, "auto" | "rules" | "json-schema" | "example") {
        return Err(format!(
            "schema_format must be 'auto', 'rules', 'json-schema' or 'example' (got '{schema_format}')"
        ));
    }
    if schema.trim().is_empty() {
        return Err(
            "schema is empty — declare at least one key, e.g. 'PORT=required|port', or set output='schema' to generate a starter schema from the .env".into(),
        );
    }

    let mut issues: Vec<Issue> = Vec::new();

    let effective_format = if schema_format == "auto" {
        if schema.trim_start().starts_with('{') {
            "json-schema"
        } else {
            "rules"
        }
    } else {
        schema_format
    };

    let decls = match effective_format {
        "json-schema" => parse_json_schema(schema)?,
        "example" => parse_example_schema(schema),
        _ => parse_rules_schema(schema, &mut issues)?,
    };
    if decls.is_empty() {
        return Err(format!(
            "schema declares no keys (parsed as {effective_format}) — declare at least one, e.g. 'PORT=required|port'"
        ));
    }

    // Parse the .env document. Duplicates are last-wins (that is what dotenv
    // loaders do), so the LAST definition is the one validated.
    let (entries, dup_lines) = parse_env(env);

    for (key, first, dup) in dup_lines {
        issues.push(Issue {
            line: dup,
            key: key.clone(),
            severity: Severity::Warning,
            rule: "duplicate-key",
            message: format!(
                "'{key}' is defined again on line {dup} (first on line {first}); the last value is the one validated"
            ),
        });
    }

    for decl in &decls {
        match entries.get(&decl.key) {
            None => report_missing(decl, 0, &mut issues),
            Some(e) if empty_is_missing && e.value.is_empty() => {
                report_missing(decl, e.line, &mut issues)
            }
            Some(e) => check_value(decl, e, &mut issues),
        }
    }

    if unknown_keys != "ignore" {
        let declared: HashSet<&str> = decls.iter().map(|d| d.key.as_str()).collect();
        let mut extras: Vec<(&String, &EnvEntry)> = entries
            .iter()
            .filter(|(k, _)| !declared.contains(k.as_str()))
            .collect();
        extras.sort_by_key(|(_, e)| e.line);
        let severity = if unknown_keys == "error" {
            Severity::Error
        } else {
            Severity::Warning
        };
        for (key, e) in extras {
            issues.push(Issue {
                line: e.line,
                key: key.clone(),
                severity,
                rule: "unknown-key",
                message: format!(
                    "'{key}' is set in the .env file but is not declared in the schema"
                ),
            });
        }
    }

    issues.sort_by_key(|i| i.line);

    Ok(match output {
        "json" => render_json(&issues, decls.len(), entries.len()),
        "markdown" => render_markdown(&issues, &decls, entries.len()),
        _ => render_report(&issues, decls.len(), entries.len()),
    })
}

fn non_empty_or<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    let t = v.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

fn report_missing(decl: &Decl, line: usize, issues: &mut Vec<Issue>) {
    let key = &decl.key;
    if !decl.required {
        return;
    }
    if let Some(d) = &decl.default {
        issues.push(Issue {
            line,
            key: key.clone(),
            severity: Severity::Warning,
            rule: "required",
            message: format!(
                "'{key}' is required but not set; the schema documents a default of '{d}', so the loader has to supply it"
            ),
        });
        return;
    }
    let how = if line == 0 {
        "is not set in the .env file".to_string()
    } else {
        format!("is set on line {line} but has an empty value")
    };
    issues.push(Issue {
        line,
        key: key.clone(),
        severity: Severity::Error,
        rule: "required",
        message: format!(
            "'{key}' is required by the schema but {how} (expected {})",
            decl.ty.as_str()
        ),
    });
}

// ---------------------------------------------------------------------------
// value checks
// ---------------------------------------------------------------------------

fn check_value(decl: &Decl, entry: &EnvEntry, issues: &mut Vec<Issue>) {
    let key = &decl.key;
    let value = &entry.value;
    let shown = display_value(key, value);
    let mut push = |severity: Severity, rule: &'static str, message: String| {
        issues.push(Issue {
            line: entry.line,
            key: key.clone(),
            severity,
            rule,
            message,
        })
    };

    let numeric = match decl.ty {
        Ty::Number => match value.parse::<f64>() {
            Ok(n) if n.is_finite() => Some(n),
            _ => {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be a number, got {shown}"),
                );
                return;
            }
        },
        Ty::Integer => match value.parse::<i64>() {
            Ok(n) => Some(n as f64),
            Err(_) => {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be a whole number, got {shown}"),
                );
                return;
            }
        },
        Ty::Port => match value.parse::<u32>() {
            Ok(n) if (1..=65535).contains(&n) => Some(n as f64),
            _ => {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be a TCP port between 1 and 65535, got {shown}"),
                );
                return;
            }
        },
        Ty::Boolean => {
            if !is_boolean(value) {
                push(
                    Severity::Error,
                    "type",
                    format!(
                        "'{key}' must be a boolean (true/false/t/f/1/0/yes/no/on/off), got {shown}"
                    ),
                );
                return;
            }
            None
        }
        Ty::Url => {
            if !is_url(value) {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be a URL with a scheme and host (e.g. https://api.example.com), got {shown}"),
                );
                return;
            }
            None
        }
        Ty::Email => {
            if !is_email(value) {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be an email address (name@example.com), got {shown}"),
                );
                return;
            }
            None
        }
        Ty::Host => {
            if !is_host(value) {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be a hostname or IP address, got {shown}"),
                );
                return;
            }
            None
        }
        Ty::Json => {
            if serde_json::from_str::<serde_json::Value>(value).is_err() {
                push(
                    Severity::Error,
                    "type",
                    format!("'{key}' must be valid JSON, got {shown}"),
                );
                return;
            }
            None
        }
        Ty::Str => None,
    };

    if let Some(allowed) = &decl.allowed {
        if !allowed.iter().any(|a| a == value) {
            push(
                Severity::Error,
                "enum",
                format!(
                    "'{key}' must be one of {}, got {shown}",
                    allowed
                        .iter()
                        .map(|a| format!("'{a}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
    }

    match (decl.min, numeric) {
        (Some(min), Some(n)) if n < min => push(
            Severity::Error,
            "min",
            format!("'{key}' must be at least {}, got {shown}", trim_num(min)),
        ),
        (Some(min), None) if (value.chars().count() as f64) < min => push(
            Severity::Error,
            "min",
            format!(
                "'{key}' must be at least {} characters long, got {}",
                trim_num(min),
                value.chars().count()
            ),
        ),
        _ => {}
    }
    match (decl.max, numeric) {
        (Some(max), Some(n)) if n > max => push(
            Severity::Error,
            "max",
            format!("'{key}' must be at most {}, got {shown}", trim_num(max)),
        ),
        (Some(max), None) if (value.chars().count() as f64) > max => push(
            Severity::Error,
            "max",
            format!(
                "'{key}' must be at most {} characters long, got {}",
                trim_num(max),
                value.chars().count()
            ),
        ),
        _ => {}
    }

    if let Some(re) = &decl.pattern {
        if !re.is_match(value) {
            push(
                Severity::Error,
                "pattern",
                format!("'{key}' must match the pattern /{}/, got {shown}", re.as_str()),
            );
        }
    }

    if decl.secure {
        if let Some(why) = insecure_reason(value) {
            push(
                Severity::Error,
                "secure",
                format!("'{key}' is not a strong secret: {why}"),
            );
        }
    }
}

/// The vocabulary the common dotenv loaders coerce to a boolean — including the
/// single-letter `t`/`f` shorthand.
fn is_boolean(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "true" | "false" | "t" | "f" | "1" | "0" | "yes" | "no" | "on" | "off"
    )
}

/// `scheme://authority` with a non-empty scheme and host.
fn is_url(v: &str) -> bool {
    let Some((scheme, rest)) = v.split_once("://") else {
        return false;
    };
    if scheme.is_empty()
        || !scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return false;
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    !host.is_empty() && !host.contains(' ')
}

fn is_email(v: &str) -> bool {
    let Some((local, domain)) = v.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !local.contains(' ')
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains(' ')
        && is_host(domain)
}

fn is_host(v: &str) -> bool {
    if v.is_empty() || v.len() > 253 {
        return false;
    }
    // IPv6 (bare or bracketed) — hex groups and colons only.
    let bare = v.trim_start_matches('[').trim_end_matches(']');
    if bare.contains(':') {
        return bare
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.');
    }
    if is_ipv4(v) {
        return true;
    }
    v.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

fn is_ipv4(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty() && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u32>().map(|n| n <= 255).unwrap_or(false)
        })
}

/// Why a value fails the `secure` rule, or `None` when it passes.
fn insecure_reason(v: &str) -> Option<String> {
    let mut missing = Vec::new();
    if v.chars().count() < 8 {
        missing.push("at least 8 characters");
    }
    if !v.chars().any(|c| c.is_lowercase()) {
        missing.push("a lowercase letter");
    }
    if !v.chars().any(|c| c.is_uppercase()) {
        missing.push("an uppercase letter");
    }
    if !v.chars().any(|c| c.is_ascii_digit()) {
        missing.push("a digit");
    }
    if !v.chars().any(|c| !c.is_alphanumeric()) {
        missing.push("a symbol");
    }
    if missing.is_empty() {
        None
    } else {
        Some(format!("needs {}", missing.join(", ")))
    }
}

/// Whether a KEY name suggests its value is a credential.
fn looks_secret(key: &str) -> bool {
    const SECRET_HINTS: [&str; 8] = [
        "SECRET", "TOKEN", "PASSWORD", "PASSWD", "APIKEY", "_KEY", "AUTH", "CREDENTIAL",
    ];
    let upper = key.to_ascii_uppercase();
    upper == "KEY" || SECRET_HINTS.iter().any(|h| upper.contains(h))
}

/// Values of secret-looking keys are masked everywhere they are echoed, so a
/// report can be pasted into an issue or CI log safely.
fn display_value(key: &str, value: &str) -> String {
    if looks_secret(key) {
        return format!("a masked value of {} characters", value.chars().count());
    }
    let shown: String = value.chars().take(40).collect();
    if shown.chars().count() < value.chars().count() {
        format!("'{shown}…'")
    } else {
        format!("'{shown}'")
    }
}

fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// ---------------------------------------------------------------------------
// .env parsing
// ---------------------------------------------------------------------------

/// Parse a `.env` document into last-wins `KEY -> entry`, plus the duplicate
/// definitions found as `(key, first_line, duplicate_line)`.
fn parse_env(env: &str) -> (HashMap<String, EnvEntry>, Vec<(String, usize, usize)>) {
    let mut map: HashMap<String, EnvEntry> = HashMap::new();
    let mut dups = Vec::new();
    for (idx, raw) in env.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value = unquote(rest.trim());
        if let Some(prev) = map.get(&key) {
            dups.push((key.clone(), prev.line, lineno));
        }
        map.insert(
            key,
            EnvEntry { line: lineno, value },
        );
    }
    (map, dups)
}

/// Strip surrounding quotes (or a trailing ` # comment` on an unquoted value).
fn unquote(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(q @ ('"' | '\'')) => {
            let rest: String = chars.collect();
            match rest.find(q) {
                Some(end) => rest[..end].to_string(),
                // Unterminated quote: keep everything after the opening quote.
                None => rest,
            }
        }
        _ => {
            let cut = raw.find(" #").map(|i| &raw[..i]).unwrap_or(raw);
            cut.trim_end().to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// schema parsing — rules dialect
// ---------------------------------------------------------------------------

fn parse_rules_schema(schema: &str, issues: &mut Vec<Issue>) -> Result<Vec<Decl>, String> {
    let mut decls: Vec<Decl> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (idx, raw) in schema.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // A trailing ` # note` documents the variable (same inline-comment
        // convention as the .env parser); it is kept for the markdown docs
        // table rather than tokenized as a rule.
        let (line, desc) = match line.split_once(" #") {
            Some((l, note)) => {
                let note = note.trim_start_matches('#').trim();
                (
                    l.trim_end(),
                    (!note.is_empty()).then(|| note.to_string()),
                )
            }
            None => (line, None),
        };
        let (key, rules) = match line.split_once('=') {
            Some((k, r)) => (k.trim(), r.trim()),
            None => (line, ""),
        };
        let key = key.strip_prefix("export ").unwrap_or(key).trim();
        if key.is_empty() {
            return Err(format!(
                "schema line {lineno}: missing a key name before '=' (expected KEY=rule|rule)"
            ));
        }
        if !seen.insert(key.to_string()) {
            issues.push(Issue {
                line: 0,
                key: key.to_string(),
                severity: Severity::Warning,
                rule: "duplicate-declaration",
                message: format!(
                    "'{key}' is declared more than once in the schema (line {lineno}); the last declaration wins"
                ),
            });
            decls.retain(|d| d.key != key);
        }

        let mut decl = Decl::new(key);
        decl.desc = desc;
        let tokens: Vec<&str> = if rules.is_empty() {
            Vec::new()
        } else {
            rules.split('|').map(|t| t.trim()).collect()
        };
        if tokens.iter().all(|t| t.is_empty()) {
            // `KEY=` with no rules — the plain ".env.example" shape: required.
            decl.required = true;
        }
        for token in tokens {
            if token.is_empty() {
                continue;
            }
            let (name, arg) = match token.split_once(':') {
                Some((n, a)) => (n.trim(), Some(a.trim())),
                None => (token, None),
            };
            match name {
                "required" => decl.required = true,
                "optional" => decl.required = false,
                "secure" => decl.secure = true,
                "enum" => {
                    let arg = arg.unwrap_or("");
                    let vals: Vec<String> = arg
                        .split(',')
                        .map(|v| strip_quotes(v.trim()).to_string())
                        .filter(|v| !v.is_empty())
                        .collect();
                    if vals.is_empty() {
                        return Err(format!(
                            "schema line {lineno}: 'enum' needs a comma-separated list of allowed values, e.g. enum:dev,staging,prod"
                        ));
                    }
                    decl.allowed = Some(vals);
                }
                "min" | "max" => {
                    let arg = arg.unwrap_or("");
                    let n: f64 = strip_quotes(arg).parse().map_err(|_| {
                        format!("schema line {lineno}: '{name}' needs a number, got '{arg}'")
                    })?;
                    if name == "min" {
                        decl.min = Some(n);
                    } else {
                        decl.max = Some(n);
                    }
                }
                "pattern" | "regex" => {
                    // Re-join on ':' — a regex may legitimately contain colons.
                    let arg = token.split_once(':').map(|(_, a)| a).unwrap_or("");
                    let arg = strip_quotes(arg.trim());
                    let re = Regex::new(arg).map_err(|e| {
                        format!("schema line {lineno}: invalid regular expression '{arg}' ({e})")
                    })?;
                    decl.pattern = Some(re);
                }
                "default" => {
                    let arg = token.split_once(':').map(|(_, a)| a).unwrap_or("");
                    decl.default = Some(strip_quotes(arg.trim()).to_string());
                }
                other => match Ty::parse(other) {
                    Some(ty) => decl.ty = ty,
                    None => issues.push(Issue {
                        line: 0,
                        key: key.to_string(),
                        severity: Severity::Warning,
                        rule: "unknown-rule",
                        message: format!(
                            "schema line {lineno}: unknown rule '{other}' for '{key}' — supported: {SUPPORTED_RULES}"
                        ),
                    }),
                },
            }
        }
        decls.push(decl);
    }
    Ok(decls)
}

fn strip_quotes(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// schema parsing — .env.example dialect
// ---------------------------------------------------------------------------

fn parse_example_schema(schema: &str) -> Vec<Decl> {
    let (entries, _) = parse_env(schema);
    let mut decls: Vec<Decl> = entries
        .into_iter()
        .map(|(key, _entry)| {
            let mut d = Decl::new(&key);
            d.required = true;
            d
        })
        .collect();
    decls.sort_by(|a, b| a.key.cmp(&b.key));
    decls
}

// ---------------------------------------------------------------------------
// schema parsing — JSON Schema dialect
// ---------------------------------------------------------------------------

fn parse_json_schema(schema: &str) -> Result<Vec<Decl>, String> {
    let root: serde_json::Value = serde_json::from_str(schema)
        .map_err(|e| format!("schema is not valid JSON: {e}"))?;
    let props = root
        .get("properties")
        .and_then(|p| p.as_object())
        .ok_or_else(|| {
            "JSON Schema needs a 'properties' object, e.g. {\"type\":\"object\",\"required\":[\"PORT\"],\"properties\":{\"PORT\":{\"type\":\"integer\"}}}"
                .to_string()
        })?;
    let required: HashSet<&str> = root
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut decls = Vec::new();
    for (key, spec) in props {
        let mut decl = Decl::new(key);
        decl.required = required.contains(key.as_str());
        let ty_name = spec
            .get("type")
            .and_then(|t| t.as_str())
            .or_else(|| {
                spec.get("type")
                    .and_then(|t| t.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("string");
        decl.ty = match ty_name {
            "integer" => Ty::Integer,
            "number" => Ty::Number,
            "boolean" => Ty::Boolean,
            "object" | "array" => Ty::Json,
            _ => Ty::Str,
        };
        if decl.ty == Ty::Str {
            if let Some(fmt) = spec.get("format").and_then(|f| f.as_str()) {
                decl.ty = match fmt {
                    "uri" | "url" | "uri-reference" => Ty::Url,
                    "email" | "idn-email" => Ty::Email,
                    "hostname" | "idn-hostname" | "ipv4" | "ipv6" => Ty::Host,
                    _ => Ty::Str,
                };
            }
        }
        if let Some(vals) = spec.get("enum").and_then(|e| e.as_array()) {
            let vals: Vec<String> = vals.iter().map(json_scalar_to_string).collect();
            if !vals.is_empty() {
                decl.allowed = Some(vals);
            }
        }
        // `const` is a one-value enum in JSON Schema; treat it as such.
        if let Some(c) = spec.get("const") {
            decl.allowed = Some(vec![json_scalar_to_string(c)]);
        }
        if let Some(d) = spec.get("description").and_then(|d| d.as_str()) {
            if !d.trim().is_empty() {
                decl.desc = Some(d.trim().to_string());
            }
        }
        // `minimum`/`maximum` bound numeric values; `minLength`/`maxLength`
        // bound string length — both land in the same slot because the checker
        // already picks value-vs-length by the declared type.
        for k in ["minimum", "minLength"] {
            if let Some(n) = spec.get(k).and_then(|v| v.as_f64()) {
                decl.min = Some(n);
            }
        }
        for k in ["maximum", "maxLength"] {
            if let Some(n) = spec.get(k).and_then(|v| v.as_f64()) {
                decl.max = Some(n);
            }
        }
        if let Some(p) = spec.get("pattern").and_then(|v| v.as_str()) {
            decl.pattern = Some(Regex::new(p).map_err(|e| {
                format!("property '{key}': invalid regular expression '{p}' ({e})")
            })?);
        }
        if let Some(d) = spec.get("default") {
            decl.default = Some(json_scalar_to_string(d));
        }
        decls.push(decl);
    }
    decls.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(decls)
}

fn json_scalar_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// schema generation (output = "schema")
// ---------------------------------------------------------------------------

/// Write a starter rules schema for the `.env` in `env`: every key becomes
/// required and its type is inferred from the value it currently holds, so the
/// generated schema passes against the same file and is a base to tighten.
fn generate_schema(env: &str) -> Result<String, String> {
    let (entries, _) = parse_env(env);
    if entries.is_empty() {
        return Err(
            "the .env is empty — paste at least one KEY=VALUE line to generate a schema from"
                .into(),
        );
    }
    let mut rows: Vec<(usize, String, String)> = entries
        .into_iter()
        .map(|(key, e)| (e.line, key, e.value))
        .collect();
    rows.sort_by_key(|(line, _, _)| *line);

    let mut out = String::from(
        "# Starter schema generated from the .env — every key is required and its\n\
         # type was inferred from the value it currently holds. Tighten it by hand:\n\
         # add enum:a,b for fixed choices, min:N/max:N bounds, pattern:REGEX, or\n\
         # 'secure' for credentials, and change 'required' to 'optional' where a key\n\
         # may be absent.\n",
    );
    for (_, key, value) in rows {
        let rules = match infer_type(&key, &value) {
            Some(ty) => format!("required|{}", ty.as_str()),
            None => "required".to_string(),
        };
        let note = if value.is_empty() {
            "  # no value set — type could not be inferred"
        } else if looks_secret(&key) {
            "  # credential — consider adding |secure or |min:N"
        } else {
            ""
        };
        out.push_str(&format!("{key}={rules}{note}\n"));
    }
    out.pop();
    Ok(out)
}

/// The narrowest type a concrete `.env` value satisfies, or `None` when the
/// value is empty and nothing can be told from it.
fn infer_type(key: &str, value: &str) -> Option<Ty> {
    if value.is_empty() {
        return None;
    }
    if is_boolean(value) && !value.chars().all(|c| c.is_ascii_digit()) {
        return Some(Ty::Boolean);
    }
    if let Ok(n) = value.parse::<u32>() {
        let porty = key.to_ascii_uppercase().contains("PORT");
        return Some(if porty && (1..=65535).contains(&n) {
            Ty::Port
        } else {
            Ty::Integer
        });
    }
    if value.parse::<i64>().is_ok() {
        return Some(Ty::Integer);
    }
    if value.parse::<f64>().is_ok_and(|n| n.is_finite()) {
        return Some(Ty::Number);
    }
    if is_url(value) {
        return Some(Ty::Url);
    }
    if is_email(value) {
        return Some(Ty::Email);
    }
    if (value.starts_with('{') || value.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(value).is_ok()
    {
        return Some(Ty::Json);
    }
    if key.to_ascii_uppercase().contains("HOST") && is_host(value) {
        return Some(Ty::Host);
    }
    Some(Ty::Str)
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_report(issues: &[Issue], declared: usize, file_keys: usize) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;

    let verdict = if errors == 0 { "PASSED" } else { "FAILED" };
    let mut out = format!(
        ".env schema check: {verdict} — {errors} {}, {warnings} {} ({declared} declared {}, {file_keys} {} in the file)\n",
        plural(errors, "error", "errors"),
        plural(warnings, "warning", "warnings"),
        plural(declared, "key", "keys"),
        plural(file_keys, "key", "keys"),
    );
    if issues.is_empty() {
        out.push_str("  every declared key is present and valid.");
        return out;
    }
    for it in issues {
        let where_ = if it.line == 0 {
            "file".to_string()
        } else {
            format!("line {}", it.line)
        };
        out.push_str(&format!(
            "  {:<8}  {:<7}  {:<22}  {}\n",
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

/// The same findings as `report`, as GitHub-flavoured Markdown: a verdict, an
/// issue table, and a documentation table of every declared variable — sized
/// for a PR comment or a CI job summary.
fn render_markdown(issues: &[Issue], decls: &[Decl], file_keys: usize) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;
    let verdict = if errors == 0 { "PASSED" } else { "FAILED" };

    let mut out = format!("## .env schema check: {verdict}\n\n");
    out.push_str(&format!(
        "{errors} {}, {warnings} {} — {} declared {}, {file_keys} {} in the file.\n",
        plural(errors, "error", "errors"),
        plural(warnings, "warning", "warnings"),
        decls.len(),
        plural(decls.len(), "key", "keys"),
        plural(file_keys, "key", "keys"),
    ));

    if issues.is_empty() {
        out.push_str("\nEvery declared key is present and valid.\n");
    } else {
        out.push_str("\n| Line | Key | Severity | Rule | Message |\n| --- | --- | --- | --- | --- |\n");
        for it in issues {
            let where_ = if it.line == 0 {
                "—".to_string()
            } else {
                it.line.to_string()
            };
            out.push_str(&format!(
                "| {} | `{}` | {} | `{}` | {} |\n",
                where_,
                it.key,
                it.severity.as_str(),
                it.rule,
                md_cell(&it.message)
            ));
        }
    }

    out.push_str("\n### Declared variables\n\n| Variable | Required | Type | Constraints | Notes |\n| --- | --- | --- | --- | --- |\n");
    for d in decls {
        let constraints = d.constraints();
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            d.key,
            if d.required { "yes" } else { "no" },
            d.ty.as_str(),
            if constraints.is_empty() {
                "—".to_string()
            } else {
                md_cell(&constraints)
            },
            d.desc.as_deref().map(md_cell).unwrap_or_else(|| "—".into()),
        ));
    }
    out.pop();
    out
}

/// Escape the characters that would break out of a Markdown table cell.
fn md_cell(v: &str) -> String {
    v.replace('|', "\\|").replace('\n', " ")
}

fn render_json(issues: &[Issue], declared: usize, file_keys: usize) -> String {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues.len() - errors;
    let arr: Vec<serde_json::Value> = issues
        .iter()
        .map(|it| {
            serde_json::json!({
                "line": if it.line == 0 { serde_json::Value::Null } else { serde_json::json!(it.line) },
                "key": it.key,
                "severity": it.severity.as_str(),
                "rule": it.rule,
                "message": it.message,
            })
        })
        .collect();
    let val = serde_json::json!({
        "ok": errors == 0,
        "declared_keys": declared,
        "file_keys": file_keys,
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

    fn report(env: &str, schema: &str) -> String {
        validate(env, schema, "auto", "warn", true, "report").unwrap()
    }

    #[test]
    fn clean_file_passes() {
        let out = report(
            "PORT=8080\nNODE_ENV=production\n",
            "PORT=required|port\nNODE_ENV=required|enum:development,production\n",
        );
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
        assert!(out.contains("2 declared keys, 2 keys in the file"), "got: {out}");
        assert!(out.contains("every declared key is present and valid"), "got: {out}");
    }

    #[test]
    fn missing_required_key_is_an_error() {
        let out = report("PORT=8080\n", "PORT=required|port\nDATABASE_URL=required|url\n");
        assert!(out.starts_with(".env schema check: FAILED"), "got: {out}");
        assert!(out.contains("required"), "got: {out}");
        assert!(out.contains("'DATABASE_URL' is required"), "got: {out}");
    }

    #[test]
    fn wrong_type_is_an_error() {
        let out = report("PORT=abc\n", "PORT=required|port\n");
        assert!(out.contains("must be a TCP port between 1 and 65535"), "got: {out}");
        let out = report("RETRIES=1.5\n", "RETRIES=integer\n");
        assert!(out.contains("must be a whole number"), "got: {out}");
        let out = report("API_URL=not-a-url\n", "API_URL=url\n");
        assert!(out.contains("must be a URL"), "got: {out}");
        let out = report("ADMIN=root@localhost\n", "ADMIN=email\n");
        assert!(out.contains("must be an email address"), "got: {out}");
        let out = report("FLAGS={oops}\n", "FLAGS=json\n");
        assert!(out.contains("must be valid JSON"), "got: {out}");
        let out = report("DEBUG=maybe\n", "DEBUG=boolean\n");
        assert!(out.contains("must be a boolean"), "got: {out}");
        let out = report("DB_HOST=bad host\n", "DB_HOST=host\n");
        assert!(out.contains("must be a hostname or IP address"), "got: {out}");
    }

    #[test]
    fn valid_values_of_every_type_pass() {
        let out = report(
            "S=text\nN=1.5\nI=42\nB=yes\nP=5432\nU=https://api.example.com/v1\nE=ops@example.com\nH=127.0.0.1\nJ={\"a\":1}\n",
            "S=string\nN=number\nI=integer\nB=boolean\nP=port\nU=url\nE=email\nH=host\nJ=json\n",
        );
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn enum_rejects_a_value_outside_the_list() {
        let out = report("NODE_ENV=staging\n", "NODE_ENV=required|enum:development,production\n");
        assert!(out.contains("must be one of 'development', 'production'"), "got: {out}");
    }

    #[test]
    fn min_max_bound_numbers_and_string_length() {
        let out = report("PORT=80\n", "PORT=number|min:1024|max:65535\n");
        assert!(out.contains("must be at least 1024"), "got: {out}");
        let out = report("API_KEY=short\n", "API_KEY=min:32\n");
        assert!(out.contains("must be at least 32 characters long"), "got: {out}");
        let out = report("NAME=abcdef\n", "NAME=max:3\n");
        assert!(out.contains("must be at most 3 characters long"), "got: {out}");
    }

    #[test]
    fn pattern_and_secure_rules() {
        let out = report("RELEASE=v1\n", "RELEASE=pattern:^v[0-9]+\\.[0-9]+\\.[0-9]+$\n");
        assert!(out.contains("must match the pattern"), "got: {out}");
        let ok = report("RELEASE=v1.2.3\n", "RELEASE=pattern:^v[0-9]+\\.[0-9]+\\.[0-9]+$\n");
        assert!(ok.starts_with(".env schema check: PASSED"), "got: {ok}");
        let out = report("DB_PASSWORD=hunter2\n", "DB_PASSWORD=required|secure\n");
        assert!(out.contains("not a strong secret"), "got: {out}");
        // The failing value itself must never be echoed for a secret-looking key.
        assert!(!out.contains("hunter2"), "got: {out}");
    }

    #[test]
    fn unknown_keys_warn_by_default_and_can_error_or_be_ignored() {
        let out = report("PORT=80\nEXTRA=1\n", "PORT=number\n");
        assert!(out.contains("unknown-key"), "got: {out}");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");

        let strict = validate("PORT=80\nEXTRA=1\n", "PORT=number", "auto", "error", true, "report")
            .unwrap();
        assert!(strict.starts_with(".env schema check: FAILED"), "got: {strict}");

        let quiet = validate("PORT=80\nEXTRA=1\n", "PORT=number", "auto", "ignore", true, "report")
            .unwrap();
        assert!(!quiet.contains("unknown-key"), "got: {quiet}");
    }

    #[test]
    fn empty_value_counts_as_missing_unless_disabled() {
        let out = report("TOKEN=\n", "TOKEN=required\n");
        assert!(out.contains("has an empty value"), "got: {out}");
        let kept = validate("TOKEN=\n", "TOKEN=required|min:3", "auto", "warn", false, "report")
            .unwrap();
        assert!(kept.contains("at least 3 characters"), "got: {kept}");
    }

    #[test]
    fn bare_key_line_means_required() {
        let out = report("OTHER=1\n", "DATABASE_URL=\n");
        assert!(out.contains("'DATABASE_URL' is required"), "got: {out}");
    }

    #[test]
    fn example_format_treats_every_key_as_required() {
        let out = validate(
            "PORT=8080\n",
            "# copy me\nPORT=8080\nDATABASE_URL=postgres://localhost/app\n",
            "example",
            "warn",
            true,
            "report",
        )
        .unwrap();
        assert!(out.contains("'DATABASE_URL' is required"), "got: {out}");
        assert!(!out.contains("'PORT' is required"), "got: {out}");
    }

    #[test]
    fn json_schema_is_auto_detected() {
        let schema = r#"{"type":"object","required":["PORT","NODE_ENV"],"properties":{
            "PORT":{"type":"integer","minimum":1,"maximum":65535},
            "NODE_ENV":{"type":"string","enum":["dev","prod"]},
            "API_URL":{"type":"string","format":"uri"}}}"#;
        let out = report("PORT=99999\nNODE_ENV=dev\nAPI_URL=https://x.example.com\n", schema);
        assert!(out.contains("must be at most 65535"), "got: {out}");
        let ok = report("PORT=8080\nNODE_ENV=dev\n", schema);
        assert!(ok.starts_with(".env schema check: PASSED"), "got: {ok}");
    }

    #[test]
    fn json_schema_pattern_and_minlength_apply() {
        let schema = r#"{"properties":{"API_KEY":{"type":"string","minLength":10,"pattern":"^sk-"}},"required":["API_KEY"]}"#;
        let out = report("API_KEY=sk-1\n", schema);
        assert!(out.contains("at least 10 characters"), "got: {out}");
        let out2 = report("API_KEY=zz-1234567890\n", schema);
        assert!(out2.contains("must match the pattern"), "got: {out2}");
    }

    #[test]
    fn quotes_export_prefix_and_comments_are_understood() {
        let out = report(
            "# comment\nexport GREETING=\"hello world\"\nPORT=8080 # inline\n",
            "GREETING=required\nPORT=required|port\n",
        );
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn duplicate_key_warns_and_validates_the_last_value() {
        let out = report("PORT=80\nPORT=abc\n", "PORT=required|port\n");
        assert!(out.contains("duplicate-key"), "got: {out}");
        assert!(out.contains("must be a TCP port"), "got: {out}");
    }

    #[test]
    fn unknown_rule_warns_but_still_validates() {
        let out = report("PORT=80\n", "PORT=required|bogus\n");
        assert!(out.contains("unknown-rule"), "got: {out}");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn documented_default_downgrades_a_missing_required_key() {
        let out = report("OTHER=1\n", "PORT=required|number|default:3000\n");
        assert!(out.contains("documents a default of '3000'"), "got: {out}");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn optional_keys_may_be_absent() {
        let out = report("PORT=80\n", "PORT=required|port\nSENTRY_DSN=url\n");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn json_output_shape() {
        let out = validate(
            "PORT=abc\nEXTRA=1\n",
            "PORT=required|port",
            "rules",
            "warn",
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], serde_json::json!(false));
        assert_eq!(v["declared_keys"], serde_json::json!(1));
        assert_eq!(v["file_keys"], serde_json::json!(2));
        assert_eq!(v["error_count"], serde_json::json!(1));
        assert_eq!(v["warning_count"], serde_json::json!(1));
        assert_eq!(v["issues"][0]["key"], serde_json::json!("PORT"));
        assert_eq!(v["issues"][0]["rule"], serde_json::json!("type"));
        assert_eq!(v["issues"][0]["line"], serde_json::json!(1));
    }

    #[test]
    fn markdown_output_lists_issues_and_documents_every_variable() {
        let out = validate(
            "PORT=99999\n",
            "PORT=required|port # the HTTP port to bind\nAPI_KEY=required|min:32|secure\n",
            "auto",
            "warn",
            true,
            "markdown",
        )
        .unwrap();
        assert!(out.starts_with("## .env schema check: FAILED"), "got: {out}");
        // Issue table.
        assert!(out.contains("| Line | Key | Severity | Rule | Message |"), "got: {out}");
        assert!(out.contains("| 1 | `PORT` | error | `type` |"), "got: {out}");
        // Docs table carries the trailing-comment note and the constraint list.
        assert!(out.contains("### Declared variables"), "got: {out}");
        assert!(out.contains("the HTTP port to bind"), "got: {out}");
        assert!(out.contains("length ≥ 32; strong secret"), "got: {out}");
    }

    #[test]
    fn a_trailing_comment_documents_a_rule_line_instead_of_being_parsed() {
        // The note must NOT be tokenized as a rule…
        let out = report("PORT=8080\n", "PORT=required|port # bind port");
        assert!(!out.contains("unknown-rule"), "got: {out}");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
        // …and a bare key line with a note still means required.
        let out = report("OTHER=1\n", "DATABASE_URL= # postgres connection string");
        assert!(out.contains("'DATABASE_URL' is required"), "got: {out}");
    }

    #[test]
    fn schema_output_generates_a_starter_schema_that_passes() {
        let env = "# app\nPORT=8080\nDEBUG=true\nAPI_URL=https://api.example.com\nADMIN=ops@example.com\nRETRIES=3\nRATIO=0.5\nFLAGS={\"a\":1}\nDB_HOST=db.internal\nAPI_KEY=sk-abc\nEMPTY=\n";
        let generated = validate(env, "", "auto", "warn", true, "schema").unwrap();
        assert!(generated.contains("PORT=required|port"), "got: {generated}");
        assert!(generated.contains("DEBUG=required|boolean"), "got: {generated}");
        assert!(generated.contains("API_URL=required|url"), "got: {generated}");
        assert!(generated.contains("ADMIN=required|email"), "got: {generated}");
        assert!(generated.contains("RETRIES=required|integer"), "got: {generated}");
        assert!(generated.contains("RATIO=required|number"), "got: {generated}");
        assert!(generated.contains("FLAGS=required|json"), "got: {generated}");
        assert!(generated.contains("DB_HOST=required|host"), "got: {generated}");
        assert!(generated.contains("credential — consider adding |secure"), "got: {generated}");
        assert!(generated.contains("EMPTY=required  # no value set"), "got: {generated}");

        // Round-trip: the generated schema validates the file it came from,
        // and only the intentionally-empty key is flagged.
        let out = report(env, &generated);
        assert!(out.contains("'EMPTY' is required"), "got: {out}");
        assert_eq!(out.matches("error").count(), 2, "got: {out}"); // header tally + the EMPTY line
    }

    #[test]
    fn schema_output_needs_a_non_empty_env() {
        let err = validate("# only a comment\n", "", "auto", "warn", true, "schema").unwrap_err();
        assert!(err.contains("the .env is empty"), "got: {err}");
    }

    #[test]
    fn boolean_accepts_the_single_letter_shorthand() {
        let out = report("DEBUG=t\nQUIET=F\n", "DEBUG=boolean\nQUIET=boolean\n");
        assert!(out.starts_with(".env schema check: PASSED"), "got: {out}");
    }

    #[test]
    fn json_schema_const_and_description_are_honoured() {
        let schema = r#"{"required":["STAGE"],"properties":{"STAGE":{"const":"prod","description":"deploy target"}}}"#;
        let out = report("STAGE=dev\n", schema);
        assert!(out.contains("must be one of 'prod'"), "got: {out}");
        let md = validate("STAGE=prod\n", schema, "auto", "warn", true, "markdown").unwrap();
        assert!(md.contains("deploy target"), "got: {md}");
    }

    #[test]
    fn empty_schema_is_an_error() {
        let err = validate("PORT=1\n", "   ", "auto", "warn", true, "report").unwrap_err();
        assert!(err.contains("schema is empty"), "got: {err}");
    }

    #[test]
    fn invalid_output_and_options_are_errors() {
        let err = validate("A=1", "A=required", "auto", "warn", true, "xml").unwrap_err();
        assert!(err.contains("output must be"), "got: {err}");
        let err = validate("A=1", "A=required", "auto", "shout", true, "report").unwrap_err();
        assert!(err.contains("unknown_keys must be"), "got: {err}");
        let err = validate("A=1", "A=required", "yaml", "warn", true, "report").unwrap_err();
        assert!(err.contains("schema_format must be"), "got: {err}");
    }

    #[test]
    fn broken_json_schema_is_an_error() {
        let err = validate("A=1", "{\"properties\":", "auto", "warn", true, "report").unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
        let err = validate("A=1", "{\"foo\":1}", "json-schema", "warn", true, "report")
            .unwrap_err();
        assert!(err.contains("needs a 'properties' object"), "got: {err}");
    }

    #[test]
    fn invalid_pattern_is_an_error() {
        let err = validate("A=1", "A=pattern:[", "rules", "warn", true, "report").unwrap_err();
        assert!(err.contains("invalid regular expression"), "got: {err}");
    }
}
