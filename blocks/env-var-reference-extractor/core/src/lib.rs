//! env-var-reference-extractor core — find every environment-variable
//! *reference* in a pasted shell script, Dockerfile, CI config or source file
//! and report where each one is used, whether it carries a fallback default,
//! and whether anything ever defines it.
//!
//! Pure Rust (only `serde_json`, for the JSON output). No wafer/wasm-bindgen
//! deps — shared verbatim by the chat skill block, the CLI and the web page.
//!
//! This is a deterministic scanner, not a shell parser. Four reference
//! families are recognised:
//!
//! - **shell** — `$VAR`, `${VAR}`, and the parameter expansions `${VAR:-d}`,
//!   `${VAR:=d}`, `${VAR:?msg}`, `${VAR:+alt}`, `${VAR#pat}`, `${#VAR}`,
//!   `${!VAR}`. A `\$` escape and a `$$` (PID / Compose's literal `$`) are
//!   skipped, and `$1`/`$@`/`$?` are positional or special parameters rather
//!   than environment variables, so they are never reported.
//! - **dockerfile** — shell references plus `ENV`/`ARG` *definitions*.
//! - **windows** — `%VAR%` and delayed-expansion `!VAR!`, plus `set`/`setx`
//!   definitions. `%%` is batch's escaped percent and is skipped.
//! - **code** — the common library accessors: `process.env.VAR`,
//!   `import.meta.env.VAR`, `os.environ["VAR"]`, `os.getenv("VAR")`,
//!   `System.getenv("VAR")`, `os.Getenv("VAR")`, `env::var("VAR")`,
//!   `getenv("VAR")`, `ENV["VAR"]`, `$_ENV['VAR']` and friends.
//!
//! Every hit carries its 1-based line and column. Hits are grouped by variable
//! name and rendered as a name list, an aligned table, JSON, a Markdown table,
//! CSV, a `.env.example` template, or a statistics summary.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Cap on the number of individual references scanned (keeps output bounded).
pub const MAX_REFERENCES: usize = 20_000;

/// Which reference families are active for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SyntaxSet {
    /// `$VAR` / `${VAR…}` references.
    shell: bool,
    /// Dockerfile `ENV`/`ARG` definitions (references are shell-shaped).
    dockerfile: bool,
    /// `%VAR%` / `!VAR!` references and `set`/`setx` definitions.
    windows: bool,
    /// `process.env.VAR`-style library accessors.
    code: bool,
}

impl SyntaxSet {
    const fn shell_only() -> Self {
        Self { shell: true, dockerfile: false, windows: false, code: false }
    }
    const fn docker() -> Self {
        Self { shell: true, dockerfile: true, windows: false, code: false }
    }
    const fn windows() -> Self {
        Self { shell: false, dockerfile: false, windows: true, code: false }
    }
    const fn code() -> Self {
        Self { shell: false, dockerfile: false, windows: false, code: true }
    }
    const fn all() -> Self {
        Self { shell: true, dockerfile: true, windows: true, code: true }
    }
}

/// One reference occurrence found in the input.
#[derive(Debug, Clone)]
struct Hit {
    name: String,
    line: usize,
    column: usize,
    /// Generic pattern the hit matched, e.g. `${VAR:-default}` — not the raw text.
    form: &'static str,
    /// Fallback value from `${VAR:-d}` / `${VAR:=d}` / `${VAR-d}` / `${VAR=d}`.
    default: Option<String>,
}

/// A variable name plus everything learned about it.
#[derive(Debug, Clone)]
struct VarEntry {
    name: String,
    count: usize,
    lines: Vec<usize>,
    forms: Vec<&'static str>,
    default: Option<String>,
    first_line: usize,
    first_column: usize,
    /// `"source"` (assigned in the pasted input) or `"list"` (in `defined`).
    defined_in: Option<&'static str>,
    defined_at_line: Option<usize>,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Library accessors written as `prefix.VAR`.
const CODE_DOT_ACCESSORS: [(&str, &str); 2] = [
    ("process.env.", "process.env.VAR"),
    ("import.meta.env.", "import.meta.env.VAR"),
];

/// Library accessors written as `prefix("VAR")` or `prefix["VAR"]`. Longest
/// prefix first so `os.environ.get` wins over `os.environ` and `os.getenv`
/// wins over the bare `getenv` at the tail.
const CODE_CALL_ACCESSORS: [(&str, &str, &str); 18] = [
    ("Environment.GetEnvironmentVariable", "Environment.GetEnvironmentVariable(\"VAR\")", "Environment.GetEnvironmentVariable[\"VAR\"]"),
    ("std::env::var_os", "std::env::var_os(\"VAR\")", "std::env::var_os[\"VAR\"]"),
    ("std::env::var", "std::env::var(\"VAR\")", "std::env::var[\"VAR\"]"),
    ("import.meta.env", "import.meta.env(\"VAR\")", "import.meta.env[\"VAR\"]"),
    ("os.environ.get", "os.environ.get(\"VAR\")", "os.environ.get[\"VAR\"]"),
    ("Environment.get", "Environment.get(\"VAR\")", "Environment.get[\"VAR\"]"),
    ("System.getenv", "System.getenv(\"VAR\")", "System.getenv[\"VAR\"]"),
    ("process.env", "process.env(\"VAR\")", "process.env[\"VAR\"]"),
    ("env::var_os", "env::var_os(\"VAR\")", "env::var_os[\"VAR\"]"),
    ("os.environ", "os.environ(\"VAR\")", "os.environ[\"VAR\"]"),
    ("os.Getenv", "os.Getenv(\"VAR\")", "os.Getenv[\"VAR\"]"),
    ("ENV.fetch", "ENV.fetch(\"VAR\")", "ENV.fetch[\"VAR\"]"),
    ("os.getenv", "os.getenv(\"VAR\")", "os.getenv[\"VAR\"]"),
    ("env::var", "env::var(\"VAR\")", "env::var[\"VAR\"]"),
    ("$_SERVER", "$_SERVER(\"VAR\")", "$_SERVER[\"VAR\"]"),
    ("$_ENV", "$_ENV(\"VAR\")", "$_ENV[\"VAR\"]"),
    // Bare forms last: `getenv` must not swallow `os.getenv`/`System.getenv`,
    // and Ruby's `ENV["VAR"]` must not swallow `ENV.fetch`.
    ("getenv", "getenv(\"VAR\")", "getenv[\"VAR\"]"),
    ("ENV", "ENV(\"VAR\")", "ENV[\"VAR\"]"),
];

/// Resolve the `syntax` param, auto-detecting from the input when asked.
fn resolve_syntax(syntax: &str, text: &str) -> Result<(SyntaxSet, &'static str, bool), String> {
    match syntax {
        "shell" => Ok((SyntaxSet::shell_only(), "shell", false)),
        "dockerfile" => Ok((SyntaxSet::docker(), "dockerfile", false)),
        "windows" => Ok((SyntaxSet::windows(), "windows", false)),
        "code" => Ok((SyntaxSet::code(), "code", false)),
        "all" => Ok((SyntaxSet::all(), "all", false)),
        "auto" => {
            let (set, label) = detect_syntax(text);
            Ok((set, label, true))
        }
        other => Err(format!(
            "unknown syntax '{other}' — use auto, shell, dockerfile, windows, code or all"
        )),
    }
}

/// Guess the reference family from the input's shape. First rule that matches wins.
fn detect_syntax(text: &str) -> (SyntaxSet, &'static str) {
    const DOCKER_INSTRUCTIONS: [&str; 10] = [
        "from ", "run ", "env ", "arg ", "cmd ", "entrypoint ", "copy ", "workdir ", "expose ",
        "label ",
    ];
    let mut looks_windows = false;
    let mut looks_code = false;
    for raw in text.lines() {
        let line = raw.trim();
        let lower = line.to_ascii_lowercase();
        if DOCKER_INSTRUCTIONS.iter().any(|k| lower.starts_with(k)) && lower.starts_with("from ") {
            return (SyntaxSet::docker(), "dockerfile");
        }
        if lower.starts_with("@echo off")
            || lower.starts_with("setlocal")
            || lower.starts_with("set ")
            || lower.starts_with("setx ")
            || lower.starts_with("rem ")
        {
            looks_windows = true;
        }
        if line.contains("process.env")
            || line.contains("import.meta.env")
            || line.contains("os.environ")
            || line.contains("os.getenv")
            || line.contains("os.Getenv")
            || line.contains("System.getenv")
            || line.contains("env::var")
            || line.contains("getenv(")
        {
            looks_code = true;
        }
    }
    // A Dockerfile without FROM still shows itself through ENV/ARG instructions.
    let has_docker_kv = text.lines().any(|raw| {
        let lower = raw.trim().to_ascii_lowercase();
        lower.starts_with("env ") || lower.starts_with("arg ")
    });
    if has_docker_kv && !looks_windows {
        return (SyntaxSet::docker(), "dockerfile");
    }
    if looks_windows && contains_percent_reference(text) {
        return (SyntaxSet::windows(), "windows");
    }
    if looks_code {
        return (SyntaxSet::code(), "code");
    }
    if contains_percent_reference(text) && !text.contains('$') {
        return (SyntaxSet::windows(), "windows");
    }
    (SyntaxSet::shell_only(), "shell")
}

/// Cheap `%NAME%` probe used only by auto-detection.
fn contains_percent_reference(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            let mut j = i + 1;
            if j < chars.len() && is_name_start(chars[j]) {
                while j < chars.len() && is_name_char(chars[j]) {
                    j += 1;
                }
                if j < chars.len() && chars[j] == '%' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// Index at which the line's trailing comment begins (== len when there is none).
fn comment_limit(chars: &[char], syn: SyntaxSet, skip_comments: bool) -> usize {
    let n = chars.len();
    if !skip_comments {
        return n;
    }
    let mut limit = n;
    if syn.shell || syn.dockerfile || syn.code {
        // `#` at line start or after whitespace. `${VAR#pat}` and `url#frag`
        // keep their `#` because it is preceded by a name character.
        for (i, c) in chars.iter().enumerate() {
            if *c == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
                limit = limit.min(i);
                break;
            }
        }
    }
    if syn.code {
        for i in 0..n.saturating_sub(1) {
            if chars[i] == '/' && chars[i + 1] == '/' && (i == 0 || chars[i - 1].is_whitespace()) {
                limit = limit.min(i);
                break;
            }
        }
    }
    if syn.windows {
        let lower: String = chars.iter().collect::<String>().trim_start().to_ascii_lowercase();
        if lower.starts_with("rem ") || lower.starts_with("::") {
            limit = 0;
        }
    }
    limit
}

/// Find the `}` matching the `${` whose contents start at `start`.
fn find_close_brace(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut k = start;
    while k < chars.len() {
        match chars[k] {
            '\\' => {
                k += 2;
                continue;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// Scan `$VAR` / `${VAR…}` references on one line.
fn scan_shell(chars: &[char], limit: usize, line: usize, out: &mut Vec<Hit>) {
    let mut i = 0;
    while i < limit {
        let c = chars[i];
        if c == '\\' {
            i += 2; // `\$` is a literal dollar
            continue;
        }
        if c != '$' {
            i += 1;
            continue;
        }
        if i + 1 < limit && chars[i + 1] == '$' {
            i += 2; // `$$` = PID in shell, an escaped `$` in Compose
            continue;
        }
        if i + 1 < limit && chars[i + 1] == '{' {
            let mut j = i + 2;
            // `${#VAR}` (length) and `${!VAR}` (indirect) both name a variable.
            while j < limit && (chars[j] == '#' || chars[j] == '!') {
                j += 1;
            }
            let name_start = j;
            while j < limit && is_name_char(chars[j]) {
                j += 1;
            }
            if j == name_start || !is_name_start(chars[name_start]) {
                i += 2; // `${1}`, `${@}`, `${}` — not an environment variable
                continue;
            }
            let name: String = chars[name_start..j].iter().collect();
            let close = find_close_brace(chars, i + 2);
            let (form, default) = braced_form(chars, j, limit, close);
            out.push(Hit { name, line, column: i + 1, form, default });
            i = j; // keep scanning inside the braces so `${A:-${B}}` reports B too
            continue;
        }
        if i + 1 < limit && is_name_start(chars[i + 1]) {
            let mut j = i + 1;
            while j < limit && is_name_char(chars[j]) {
                j += 1;
            }
            let name: String = chars[i + 1..j].iter().collect();
            out.push(Hit { name, line, column: i + 1, form: "$VAR", default: None });
            i = j;
            continue;
        }
        i += 1; // `$(`, `$1`, `$@`, `$?` … — step over the `$` and keep scanning
    }
}

/// Classify the expansion operator that follows a braced name, and pull out the
/// fallback value when the operator supplies one.
fn braced_form(
    chars: &[char],
    after_name: usize,
    limit: usize,
    close: Option<usize>,
) -> (&'static str, Option<String>) {
    let end = close.filter(|c| *c < limit);
    let value_from = |start: usize| -> Option<String> {
        end.filter(|c| *c > start).map(|c| chars[start..c].iter().collect())
    };
    if after_name >= limit {
        return ("${VAR}", None);
    }
    match chars[after_name] {
        '}' => ("${VAR}", None),
        ':' if after_name + 1 < limit => match chars[after_name + 1] {
            '-' | '=' => ("${VAR:-default}", value_from(after_name + 2)),
            '?' => ("${VAR:?error}", None),
            '+' => ("${VAR:+alt}", None),
            _ => ("${VAR:offset}", None),
        },
        '-' | '=' => ("${VAR:-default}", value_from(after_name + 1)),
        '?' => ("${VAR:?error}", None),
        '+' => ("${VAR:+alt}", None),
        _ => ("${VAR#pattern}", None),
    }
}

/// Scan `%VAR%` and `!VAR!` references on one line.
fn scan_windows(chars: &[char], limit: usize, line: usize, out: &mut Vec<Hit>) {
    let mut i = 0;
    while i < limit {
        let c = chars[i];
        if c != '%' && c != '!' {
            i += 1;
            continue;
        }
        if c == '%' && i + 1 < limit && chars[i + 1] == '%' {
            i += 2; // `%%` is batch's escaped percent
            continue;
        }
        let mut j = i + 1;
        if j < limit && is_name_start(chars[j]) {
            while j < limit && is_name_char(chars[j]) {
                j += 1;
            }
            if j < limit && chars[j] == c {
                let name: String = chars[i + 1..j].iter().collect();
                let form = if c == '%' { "%VAR%" } else { "!VAR!" };
                out.push(Hit { name, line, column: i + 1, form, default: None });
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
}

/// True when the character before `pos` could continue an identifier, meaning
/// the accessor we matched is really the tail of a longer name.
fn boundary_ok(line: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    match line[..pos].chars().next_back() {
        Some(c) => !(is_name_char(c) || c == '.' || c == ':' || c == '$'),
        None => true,
    }
}

/// Scan `process.env.VAR` / `os.environ["VAR"]` style accessors on one line.
fn scan_code(line: &str, limit_bytes: usize, line_no: usize, out: &mut Vec<Hit>) {
    let src = &line[..limit_bytes];
    let mut pos = 0usize;
    'outer: while pos < src.len() {
        if !src.is_char_boundary(pos) {
            pos += 1;
            continue;
        }
        let rest = &src[pos..];
        for (prefix, form) in CODE_DOT_ACCESSORS {
            if rest.starts_with(prefix) && boundary_ok(src, pos) {
                let after = &rest[prefix.len()..];
                let name: String = after.chars().take_while(|c| is_name_char(*c)).collect();
                if !name.is_empty() && is_name_start(name.chars().next().unwrap()) {
                    out.push(Hit {
                        name: name.clone(),
                        line: line_no,
                        column: src[..pos].chars().count() + 1,
                        form,
                        default: None,
                    });
                    pos += prefix.len() + name.len();
                    continue 'outer;
                }
            }
        }
        for (prefix, paren_form, bracket_form) in CODE_CALL_ACCESSORS {
            if !rest.starts_with(prefix) || !boundary_ok(src, pos) {
                continue;
            }
            let after = &rest[prefix.len()..];
            let trimmed = after.trim_start();
            let opener = match trimmed.chars().next() {
                Some(c @ ('(' | '[')) => c,
                _ => continue,
            };
            let inner = trimmed[opener.len_utf8()..].trim_start();
            let quote = match inner.chars().next() {
                Some(q @ ('"' | '\'')) => q,
                _ => continue,
            };
            let body = &inner[quote.len_utf8()..];
            let name: String = body.chars().take_while(|c| *c != quote).collect();
            if name.is_empty()
                || !is_name_start(name.chars().next().unwrap())
                || !name.chars().all(is_name_char)
            {
                continue;
            }
            out.push(Hit {
                name,
                line: line_no,
                column: src[..pos].chars().count() + 1,
                form: if opener == '(' { paren_form } else { bracket_form },
                default: None,
            });
            pos += prefix.len();
            continue 'outer;
        }
        pos += 1;
    }
}

/// Split a Dockerfile/batch instruction into whitespace-separated tokens,
/// keeping quoted runs together.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Record every variable the pasted input itself assigns.
fn scan_definitions(
    line: &str,
    line_no: usize,
    syn: SyntaxSet,
    defs: &mut BTreeMap<String, usize>,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let lower = trimmed.to_ascii_lowercase();

    if syn.dockerfile && (lower.starts_with("env ") || lower.starts_with("arg ")) {
        let rest = trimmed[4..].trim();
        let tokens = tokenize(rest);
        if lower.starts_with("arg ") {
            if let Some(first) = tokens.first() {
                let name = first.split('=').next().unwrap_or("");
                record_definition(name, line_no, defs);
            }
        } else if rest.contains('=') {
            for token in &tokens {
                if let Some((name, _)) = token.split_once('=') {
                    record_definition(name, line_no, defs);
                }
            }
        } else if let Some(first) = tokens.first() {
            record_definition(first, line_no, defs);
        }
        return;
    }

    if syn.windows && (lower.starts_with("set ") || lower.starts_with("setx ")) {
        let is_setx = lower.starts_with("setx ");
        let mut rest = trimmed[if is_setx { 5 } else { 4 }..].trim();
        while rest.starts_with('/') {
            rest = rest.split_once(char::is_whitespace).map(|(_, r)| r.trim()).unwrap_or("");
        }
        let rest = rest.trim_start_matches('"');
        if is_setx {
            record_definition(rest.split_whitespace().next().unwrap_or(""), line_no, defs);
        } else if let Some((name, _)) = rest.split_once('=') {
            record_definition(name.trim(), line_no, defs);
        }
        return;
    }

    if syn.shell {
        for segment in trimmed.split([';', '&', '|']) {
            let mut seg = segment.trim();
            loop {
                let before = seg;
                for kw in ["export ", "declare -x ", "declare ", "readonly ", "local ", "typeset "]
                {
                    if let Some(rest) = seg.strip_prefix(kw) {
                        seg = rest.trim_start();
                    }
                }
                if seg == before {
                    break;
                }
            }
            if let Some((name, _)) = seg.split_once('=') {
                record_definition(name.trim(), line_no, defs);
            }
        }
    }
}

/// Insert a definition if the token really is a variable name (first wins).
fn record_definition(name: &str, line_no: usize, defs: &mut BTreeMap<String, usize>) {
    let name = name.trim();
    if name.is_empty() || !is_name_start(name.chars().next().unwrap()) {
        return;
    }
    if !name.chars().all(is_name_char) {
        return;
    }
    defs.entry(name.to_string()).or_insert(line_no);
}

/// Parse the `defined` param: a `.env` file, a `KEY=value` list, or bare names.
fn parse_defined(defined: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for (idx, raw) in defined.lines().enumerate() {
        let line = match raw.split_once('#') {
            Some((before, _)) if before.trim_start().is_empty() => "",
            Some((before, _)) => before,
            None => raw,
        };
        let line = line.trim().trim_start_matches("export ").trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            record_definition(name, idx + 1, &mut out);
        } else {
            for token in line.split([',', ' ', '\t', ';']) {
                record_definition(token, idx + 1, &mut out);
            }
        }
    }
    out
}

/// Parse the `ignore` param into exact names and `PREFIX*` wildcards.
fn parse_ignore(ignore: &str) -> (Vec<String>, Vec<String>) {
    let mut exact = Vec::new();
    let mut prefixes = Vec::new();
    for token in ignore.split([',', ' ', '\t', '\n', ';']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match token.strip_suffix('*') {
            Some(p) if !p.is_empty() => prefixes.push(p.to_string()),
            _ => exact.push(token.to_string()),
        }
    }
    (exact, prefixes)
}

fn is_ignored(name: &str, exact: &[String], prefixes: &[String]) -> bool {
    exact.iter().any(|e| e == name) || prefixes.iter().any(|p| name.starts_with(p.as_str()))
}

/// Render up to 20 line numbers, then a `+N more` tail.
fn lines_display(lines: &[usize]) -> String {
    const MAX: usize = 20;
    let shown: Vec<String> = lines.iter().take(MAX).map(|l| l.to_string()).collect();
    if lines.len() > MAX {
        format!("{}, +{} more", shown.join(", "), lines.len() - MAX)
    } else {
        shown.join(", ")
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn status_of(entry: &VarEntry) -> &'static str {
    if entry.defined_in.is_some() {
        "defined"
    } else {
        "undefined"
    }
}

const EMPTY_MESSAGE: &str = "No environment-variable references found.";

/// Scan `text` for environment-variable references and render the report.
///
/// * `syntax` — `auto` | `shell` | `dockerfile` | `windows` | `code` | `all`
/// * `output` — `names` | `table` | `json` | `markdown` | `csv` | `env-template` | `stats`
/// * `defined` — optional `.env` body or name list treated as already defined
#[allow(clippy::too_many_arguments)]
pub fn extract(
    text: &str,
    syntax: &str,
    output: &str,
    defined: &str,
    include_defined_in_source: bool,
    skip_comments: bool,
    ignore: &str,
    only_undefined: bool,
    sort: &str,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err(
            "text is empty — paste a shell script, Dockerfile, CI config or source file to scan"
                .to_string(),
        );
    }
    if !matches!(
        output,
        "names" | "table" | "json" | "markdown" | "csv" | "env-template" | "stats"
    ) {
        return Err(format!(
            "unknown output '{output}' — use names, table, json, markdown, csv, env-template or stats"
        ));
    }
    if !matches!(sort, "name" | "occurrences" | "first-seen") {
        return Err(format!(
            "unknown sort '{sort}' — use name, occurrences or first-seen"
        ));
    }
    let (syn, syntax_label, auto_detected) = resolve_syntax(syntax, text)?;

    let mut hits: Vec<Hit> = Vec::new();
    let mut source_defs: BTreeMap<String, usize> = BTreeMap::new();
    let mut lines_scanned = 0usize;

    for (idx, raw) in text.lines().enumerate() {
        lines_scanned = idx + 1;
        let line_no = idx + 1;
        let chars: Vec<char> = raw.chars().collect();
        let limit = comment_limit(&chars, syn, skip_comments);
        if limit == 0 {
            continue;
        }
        let visible: String = chars[..limit].iter().collect();
        if syn.shell {
            scan_shell(&chars, limit, line_no, &mut hits);
        }
        if syn.windows {
            scan_windows(&chars, limit, line_no, &mut hits);
        }
        if syn.code {
            let byte_limit = visible.len();
            scan_code(&visible, byte_limit, line_no, &mut hits);
        }
        if include_defined_in_source {
            scan_definitions(&visible, line_no, syn, &mut source_defs);
        }
        if hits.len() > MAX_REFERENCES {
            return Err(format!(
                "too many references — this tool scans at most {MAX_REFERENCES} references per run; \
                 split the input into smaller files"
            ));
        }
    }

    let list_defs = parse_defined(defined);
    let (ignore_exact, ignore_prefixes) = parse_ignore(ignore);

    let mut by_name: BTreeMap<String, VarEntry> = BTreeMap::new();
    for hit in &hits {
        if is_ignored(&hit.name, &ignore_exact, &ignore_prefixes) {
            continue;
        }
        let entry = by_name.entry(hit.name.clone()).or_insert_with(|| VarEntry {
            name: hit.name.clone(),
            count: 0,
            lines: Vec::new(),
            forms: Vec::new(),
            default: None,
            first_line: hit.line,
            first_column: hit.column,
            defined_in: None,
            defined_at_line: None,
        });
        entry.count += 1;
        if !entry.lines.contains(&hit.line) {
            entry.lines.push(hit.line);
        }
        if !entry.forms.contains(&hit.form) {
            entry.forms.push(hit.form);
        }
        if entry.default.is_none() {
            if let Some(d) = &hit.default {
                entry.default = Some(d.clone());
            }
        }
    }

    for entry in by_name.values_mut() {
        entry.lines.sort_unstable();
        entry.forms.sort_unstable();
        if let Some(line) = source_defs.get(&entry.name) {
            entry.defined_in = Some("source");
            entry.defined_at_line = Some(*line);
        } else if let Some(line) = list_defs.get(&entry.name) {
            entry.defined_in = Some("list");
            entry.defined_at_line = Some(*line);
        }
    }

    let total_references: usize = by_name.values().map(|e| e.count).sum();
    let defined_in_source = by_name.values().filter(|e| e.defined_in == Some("source")).count();
    let defined_in_list = by_name.values().filter(|e| e.defined_in == Some("list")).count();
    let with_default = by_name.values().filter(|e| e.default.is_some()).count();
    let distinct_total = by_name.len();
    let undefined_total = distinct_total - defined_in_source - defined_in_list;

    let mut entries: Vec<VarEntry> = by_name.into_values().collect();
    if only_undefined {
        entries.retain(|e| e.defined_in.is_none());
    }
    match sort {
        "occurrences" => entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name))),
        "first-seen" => entries.sort_by(|a, b| {
            a.first_line
                .cmp(&b.first_line)
                .then(a.first_column.cmp(&b.first_column))
                .then(a.name.cmp(&b.name))
        }),
        _ => entries.sort_by(|a, b| a.name.cmp(&b.name)),
    }

    if output == "stats" {
        return Ok(render_stats(
            syntax_label,
            auto_detected,
            lines_scanned,
            distinct_total,
            total_references,
            defined_in_source,
            defined_in_list,
            undefined_total,
            with_default,
            &entries,
        ));
    }
    if output == "json" {
        return Ok(render_json(&entries));
    }
    if entries.is_empty() {
        return Ok(EMPTY_MESSAGE.to_string());
    }
    Ok(match output {
        "names" => entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join("\n"),
        "table" => render_table(&entries),
        "markdown" => render_markdown(&entries),
        "csv" => render_csv(&entries),
        _ => render_env_template(&entries),
    })
}

fn render_json(entries: &[VarEntry]) -> String {
    let arr: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "count": e.count,
                "lines": e.lines,
                "forms": e.forms,
                "default": e.default,
                "defined": e.defined_in.is_some(),
                "defined_in": e.defined_in,
                "defined_at_line": e.defined_at_line,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn render_table(entries: &[VarEntry]) -> String {
    let rows: Vec<[String; 5]> = entries
        .iter()
        .map(|e| {
            [
                e.name.clone(),
                e.count.to_string(),
                lines_display(&e.lines),
                e.default.clone().unwrap_or_default(),
                status_of(e).to_string(),
            ]
        })
        .collect();
    let headers = ["VARIABLE", "USES", "LINES", "DEFAULT", "STATUS"];
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let render_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i == cells.len() - 1 {
                    c.clone()
                } else {
                    format!("{:<width$}", c, width = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let mut out = vec![render_row(&header_cells)];
    for row in &rows {
        out.push(render_row(row));
    }
    out.join("\n")
}

fn render_markdown(entries: &[VarEntry]) -> String {
    let esc = |s: &str| s.replace('|', "\\|");
    let mut out = vec![
        "| Variable | Uses | Lines | Default | Status |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];
    for e in entries {
        out.push(format!(
            "| `{}` | {} | {} | {} | {} |",
            esc(&e.name),
            e.count,
            lines_display(&e.lines),
            e.default.as_deref().map(|d| format!("`{}`", esc(d))).unwrap_or_else(|| "—".to_string()),
            status_of(e)
        ));
    }
    out.join("\n")
}

fn render_csv(entries: &[VarEntry]) -> String {
    let mut out = vec!["name,uses,lines,default,status".to_string()];
    for e in entries {
        out.push(format!(
            "{},{},{},{},{}",
            csv_field(&e.name),
            e.count,
            csv_field(&lines_display(&e.lines)),
            csv_field(e.default.as_deref().unwrap_or("")),
            status_of(e)
        ));
    }
    out.join("\n")
}

fn render_env_template(entries: &[VarEntry]) -> String {
    let mut out = Vec::new();
    for e in entries {
        let plural = if e.count == 1 { "use" } else { "uses" };
        out.push(format!(
            "# {} {} on line{} {}",
            e.count,
            plural,
            if e.lines.len() == 1 { "" } else { "s" },
            lines_display(&e.lines)
        ));
        out.push(format!("{}={}", e.name, e.default.clone().unwrap_or_default()));
        out.push(String::new());
    }
    while out.last().map(|s| s.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

#[allow(clippy::too_many_arguments)]
fn render_stats(
    syntax_label: &str,
    auto_detected: bool,
    lines_scanned: usize,
    distinct: usize,
    total_references: usize,
    defined_in_source: usize,
    defined_in_list: usize,
    undefined: usize,
    with_default: usize,
    entries: &[VarEntry],
) -> String {
    let detected = if auto_detected { " (auto-detected)" } else { "" };
    let mut most: Vec<&VarEntry> = entries.iter().collect();
    most.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    let top = most
        .first()
        .map(|e| format!("{} ({})", e.name, e.count))
        .unwrap_or_else(|| "—".to_string());
    [
        format!("Syntax: {syntax_label}{detected}"),
        format!("Lines scanned: {lines_scanned}"),
        format!("Distinct variables: {distinct}"),
        format!("Total references: {total_references}"),
        format!("Defined in the pasted input: {defined_in_source}"),
        format!("Defined in the supplied list: {defined_in_list}"),
        format!("Undefined: {undefined}"),
        format!("With a fallback default: {with_default}"),
        format!("Most referenced: {top}"),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str, syntax: &str) -> String {
        extract(text, syntax, "names", "", true, true, "", false, "name").unwrap()
    }

    #[test]
    fn lists_shell_references_deduplicated_and_sorted() {
        let src = "#!/bin/sh\nHOST=${DB_HOST:-localhost}\necho \"$DB_HOST:$DB_PORT\"\n";
        assert_eq!(names(src, "shell"), "DB_HOST\nDB_PORT");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = extract("   \n\t", "auto", "names", "", true, true, "", false, "name")
            .unwrap_err();
        assert!(err.contains("text is empty"), "{err}");
    }

    #[test]
    fn unknown_output_is_an_error() {
        let err = extract("$A", "shell", "xml", "", true, true, "", false, "name").unwrap_err();
        assert!(err.contains("unknown output 'xml'"), "{err}");
    }

    #[test]
    fn unknown_syntax_and_sort_are_errors() {
        let err = extract("$A", "perl", "names", "", true, true, "", false, "name").unwrap_err();
        assert!(err.contains("unknown syntax 'perl'"), "{err}");
        let err = extract("$A", "shell", "names", "", true, true, "", false, "size").unwrap_err();
        assert!(err.contains("unknown sort 'size'"), "{err}");
    }

    #[test]
    fn positional_and_special_parameters_are_not_variables() {
        assert_eq!(names("echo $1 $@ $? $$ $# $0 ${11} $HOME", "shell"), "HOME");
    }

    #[test]
    fn escaped_dollar_is_skipped() {
        assert_eq!(names("echo \\$NOT_A_VAR $REAL", "shell"), "REAL");
    }

    #[test]
    fn comments_are_skipped_but_expansion_operators_survive() {
        let src = "VAL=${PREFIX#/opt} # $IGNORED_IN_COMMENT\n";
        assert_eq!(names(src, "shell"), "PREFIX");
        let with_comments =
            extract(src, "shell", "names", "", true, false, "", false, "name").unwrap();
        assert_eq!(with_comments, "IGNORED_IN_COMMENT\nPREFIX");
    }

    #[test]
    fn nested_default_reports_both_variables() {
        assert_eq!(names("echo ${A:-${B}}", "shell"), "A\nB");
    }

    #[test]
    fn windows_percent_and_delayed_expansion() {
        let src = "@echo off\nset TARGET=%USERPROFILE%\\out\necho !TARGET! 50%% done\n";
        assert_eq!(names(src, "windows"), "TARGET\nUSERPROFILE");
    }

    #[test]
    fn code_accessors_are_recognised() {
        let src = "const a = process.env.API_KEY;\nconst b = process.env[\"PORT\"];\nimport.meta.env.VITE_URL;\nos.environ.get('HOME_DIR')\nSystem.getenv(\"JAVA_OPT\")\n";
        assert_eq!(names(src, "code"), "API_KEY\nHOME_DIR\nJAVA_OPT\nPORT\nVITE_URL");
    }

    #[test]
    fn dockerfile_env_and_arg_count_as_definitions() {
        let src = "FROM alpine\nARG VERSION=1.0\nENV APP_HOME=/app PORT=8080\nRUN echo $VERSION $APP_HOME $PORT $MISSING\n";
        let out = extract(src, "auto", "csv", "", true, true, "", false, "name").unwrap();
        assert_eq!(
            out,
            "name,uses,lines,default,status\n\
             APP_HOME,1,4,,defined\n\
             MISSING,1,4,,undefined\n\
             PORT,1,4,,defined\n\
             VERSION,1,4,,defined"
        );
    }

    #[test]
    fn only_undefined_uses_the_supplied_defined_list() {
        let src = "echo $ALPHA $BETA $GAMMA";
        let out = extract(
            src,
            "shell",
            "names",
            "# known\nALPHA=1\nexport BETA=2\n",
            true,
            true,
            "",
            true,
            "name",
        )
        .unwrap();
        assert_eq!(out, "GAMMA");
    }

    #[test]
    fn ignore_supports_exact_names_and_wildcards() {
        let src = "echo $PATH $HOME $LC_ALL $LC_TIME $KEEP";
        let out = extract(src, "shell", "names", "", true, true, "PATH, LC_*", false, "name")
            .unwrap();
        assert_eq!(out, "HOME\nKEEP");
    }

    #[test]
    fn sort_by_occurrences_then_first_seen() {
        let src = "$B\n$A $A\n$C $A";
        let by_count =
            extract(src, "shell", "names", "", true, true, "", false, "occurrences").unwrap();
        assert_eq!(by_count, "A\nB\nC");
        let by_seen =
            extract(src, "shell", "names", "", true, true, "", false, "first-seen").unwrap();
        assert_eq!(by_seen, "B\nA\nC");
    }

    #[test]
    fn json_output_carries_lines_forms_and_defaults() {
        let src = "PORT=${PORT:-8080}\necho $PORT";
        let out = extract(src, "shell", "json", "", false, true, "", false, "name").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v,
            json!([{
                "name": "PORT",
                "count": 2,
                "lines": [1, 2],
                "forms": ["$VAR", "${VAR:-default}"],
                "default": "8080",
                "defined": false,
                "defined_in": null,
                "defined_at_line": null
            }])
        );
    }

    #[test]
    fn env_template_renders_a_dotenv_example() {
        let src = "echo ${DB_HOST:-localhost} $DB_HOST $TOKEN";
        let out =
            extract(src, "shell", "env-template", "", true, true, "", false, "name").unwrap();
        assert_eq!(
            out,
            "# 2 uses on line 1\nDB_HOST=localhost\n\n# 1 use on line 1\nTOKEN="
        );
    }

    #[test]
    fn markdown_and_table_render_status_columns() {
        let src = "API=1\necho $API $OTHER";
        let md = extract(src, "shell", "markdown", "", true, true, "", false, "name").unwrap();
        assert_eq!(
            md,
            "| Variable | Uses | Lines | Default | Status |\n\
             | --- | --- | --- | --- | --- |\n\
             | `API` | 1 | 2 | — | defined |\n\
             | `OTHER` | 1 | 2 | — | undefined |"
        );
        let table = extract(src, "shell", "table", "", true, true, "", false, "name").unwrap();
        assert_eq!(
            table,
            "VARIABLE  USES  LINES  DEFAULT  STATUS\n\
             API       1     2               defined\n\
             OTHER     1     2               undefined"
        );
    }

    #[test]
    fn stats_reports_the_detected_syntax() {
        let src = "FROM alpine\nENV PORT=1\nRUN echo $PORT $MISSING\n";
        let out = extract(src, "auto", "stats", "", true, true, "", false, "name").unwrap();
        assert!(out.starts_with("Syntax: dockerfile (auto-detected)\n"), "{out}");
        assert!(out.contains("Distinct variables: 2"), "{out}");
        assert!(out.contains("Undefined: 1"), "{out}");
    }

    #[test]
    fn no_references_returns_a_friendly_message() {
        assert_eq!(names("echo hello world", "shell"), EMPTY_MESSAGE);
        let json = extract("echo hi", "shell", "json", "", true, true, "", false, "name").unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn reference_cap_is_enforced_at_the_boundary() {
        let at_cap = "$A\n".repeat(MAX_REFERENCES);
        assert_eq!(names(&at_cap, "shell"), "A");
        let over_cap = "$A\n".repeat(MAX_REFERENCES + 1);
        let err =
            extract(&over_cap, "shell", "names", "", true, true, "", false, "name").unwrap_err();
        assert!(err.contains("too many references"), "{err}");
    }

    #[test]
    fn include_defined_in_source_can_be_turned_off() {
        let src = "PORT=8080\necho $PORT";
        let on = extract(src, "shell", "csv", "", true, true, "", false, "name").unwrap();
        assert!(on.ends_with("PORT,1,2,,defined"), "{on}");
        let off = extract(src, "shell", "csv", "", false, true, "", false, "name").unwrap();
        assert!(off.ends_with("PORT,1,2,,undefined"), "{off}");
    }

    #[test]
    fn all_syntax_scans_every_family_at_once() {
        let src = "echo $SHELL_VAR %WIN_VAR%\nprocess.env.CODE_VAR";
        assert_eq!(names(src, "all"), "CODE_VAR\nSHELL_VAR\nWIN_VAR");
    }
}
