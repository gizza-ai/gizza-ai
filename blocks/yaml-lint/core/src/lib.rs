//! yaml-lint core — lint and validate YAML: syntax errors, indentation problems,
//! duplicate keys and the usual style traps, every finding carrying a 1-based
//! line and column plus the rule id that produced it.
//!
//! Pure compute, no I/O: the same entry point backs the chat/CLI block and the
//! browser page.
//!
//! Two analysis passes feed one problem list:
//!
//!   * **Event pass** — `yaml-rust2`'s marked event parser. It gives a real
//!     parser's verdict (a `syntax` problem with the exact marker when the
//!     document doesn't parse), the document count for `---` streams, and
//!     structure-aware findings that a line scanner cannot get right:
//!     duplicate mapping keys, key ordering, empty values, and `truthy` /
//!     octal-looking scalars (only *plain*, unquoted scalars can fire, so
//!     `"yes"` and `'0755'` are correctly ignored).
//!   * **Line pass** — a quote-aware scanner for the cosmetic rules that exist
//!     between the tokens: tabs in indentation, indent width, line length,
//!     trailing spaces, blank-line runs, comment spacing, colon and hyphen
//!     spacing, the missing newline at end of file and the missing `---`.
//!     Block-scalar bodies (`|`, `>`) and the interiors of flow collections are
//!     skipped so literal text and multi-line `[...]` don't draw false hits.
//!
//! Rules are grouped into three presets (`relaxed` / `default` / `strict`) and
//! individual rules can be switched off by id, mirroring the vocabulary of the
//! reference `yamllint` rule set.

use std::collections::HashMap;

use serde_json::json;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

/// Largest document accepted. A config file is not a data set; bigger inputs are
/// rejected with a clear message instead of locking up the browser tab.
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// At most this many problems are reported; the rest are summarised.
pub const MAX_PROBLEMS: usize = 500;

// ---------------------------------------------------------------------------
// Rules & presets
// ---------------------------------------------------------------------------

/// Every rule this linter can emit, with the lowest preset level that enables it
/// (0 = relaxed, 1 = default, 2 = strict).
pub const RULES: &[(&str, u8)] = &[
    ("syntax", 0),
    ("key-duplicates", 0),
    ("indentation", 0),
    ("line-length", 1),
    ("trailing-spaces", 1),
    ("new-line-at-end-of-file", 1),
    ("empty-lines", 1),
    ("comments", 1),
    ("colons", 1),
    ("hyphens", 1),
    ("truthy", 1),
    ("octal-values", 1),
    ("document-start", 2),
    ("key-ordering", 2),
    ("empty-values", 2),
];

/// How aggressive the rule set is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Only what is actually broken: syntax, duplicate keys, indentation.
    Relaxed,
    /// Everything in `relaxed` plus the common style/robustness rules.
    Default,
    /// Everything, including the opinionated `---` / key-order / empty-value rules.
    Strict,
}

impl Preset {
    pub fn parse(s: &str) -> Result<Preset, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "default" => Ok(Preset::Default),
            "relaxed" => Ok(Preset::Relaxed),
            "strict" => Ok(Preset::Strict),
            other => Err(format!(
                "unknown preset '{other}' (use relaxed, default, or strict)"
            )),
        }
    }
    fn level(self) -> u8 {
        match self {
            Preset::Relaxed => 0,
            Preset::Default => 1,
            Preset::Strict => 2,
        }
    }
}

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Human-readable `line:col  level  message  [rule]` rows.
    Report,
    /// Machine-readable object for CI.
    Json,
}

impl ReportFormat {
    pub fn parse(s: &str) -> Result<ReportFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" | "text" => Ok(ReportFormat::Report),
            "json" => Ok(ReportFormat::Json),
            other => Err(format!(
                "unknown report_format '{other}' (use report or json)"
            )),
        }
    }
}

/// Resolved linter configuration.
#[derive(Debug, Clone)]
pub struct Options {
    pub preset: Preset,
    /// Expected indentation step in spaces (1–8).
    pub indent_spaces: usize,
    /// Maximum line length in characters; `0` disables `line-length`.
    pub max_line_length: usize,
    /// Rule ids switched off on top of the preset.
    pub disabled: Vec<String>,
    /// Report every warning as an error (the `--strict` convention).
    pub strict_warnings: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            preset: Preset::Default,
            indent_spaces: 2,
            max_line_length: 80,
            disabled: Vec::new(),
            strict_warnings: false,
        }
    }
}

impl Options {
    fn enabled(&self, rule: &str) -> bool {
        let level = RULES
            .iter()
            .find(|(id, _)| *id == rule)
            .map(|(_, lvl)| *lvl)
            .unwrap_or(0);
        level <= self.preset.level() && !self.disabled.iter().any(|d| d == rule)
    }
}

/// Parse a comma/space separated list of rule ids, rejecting unknown ones.
pub fn parse_rule_list(spec: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in spec.split([',', ' ', '\n', '\t']) {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        if !RULES.iter().any(|(r, _)| *r == id) {
            let known: Vec<&str> = RULES.iter().map(|(r, _)| *r).collect();
            return Err(format!(
                "unknown rule '{id}' (known rules: {})",
                known.join(", ")
            ));
        }
        if !out.iter().any(|k: &String| k == id) {
            out.push(id.to_string());
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Problems & report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 1-based source line.
    pub line: usize,
    /// 1-based source column.
    pub column: usize,
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Report {
    /// Whether the document parsed (no `syntax` problem).
    pub valid: bool,
    /// Number of documents in the `---` stream.
    pub documents: usize,
    /// Number of source lines.
    pub lines: usize,
    pub errors: usize,
    pub warnings: usize,
    pub problems: Vec<Problem>,
    /// True when more than `MAX_PROBLEMS` were found and the list was cut.
    pub truncated: bool,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Lint `input` and render the result — the typed entry point used by the chat
/// block and the CLI.
pub fn run(
    input: &str,
    preset: &str,
    indent_spaces: usize,
    max_line_length: usize,
    disable: &str,
    strict_warnings: bool,
    report_format: &str,
) -> Result<String, String> {
    if !(1..=8).contains(&indent_spaces) {
        return Err(format!(
            "indent_spaces must be between 1 and 8 (got {indent_spaces})"
        ));
    }
    if max_line_length > 1000 {
        return Err(format!(
            "max_line_length must be between 0 and 1000 (got {max_line_length}); 0 disables the check"
        ));
    }
    let opts = Options {
        preset: Preset::parse(preset)?,
        indent_spaces,
        max_line_length,
        disabled: parse_rule_list(disable)?,
        strict_warnings,
    };
    let format = ReportFormat::parse(report_format)?;
    let report = lint(input, &opts)?;
    Ok(render(&report, format))
}

/// String-in/string-out entry point for the browser page, where every field
/// arrives as text.
pub fn run_str(
    input: &str,
    preset: &str,
    indent_spaces: &str,
    max_line_length: &str,
    disable: &str,
    strict_warnings: &str,
    report_format: &str,
) -> Result<String, String> {
    let indent = parse_usize(indent_spaces, 2, "indent_spaces")?;
    let max_len = parse_usize(max_line_length, 80, "max_line_length")?;
    run(
        input,
        preset,
        indent,
        max_len,
        disable,
        truthy(strict_warnings),
        report_format,
    )
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_usize(raw: &str, default: usize, name: &str) -> Result<usize, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<usize>()
        .map_err(|_| format!("{name} must be a whole number (got '{t}')"))
}

/// Run both passes and assemble the report.
pub fn lint(input: &str, opts: &Options) -> Result<Report, String> {
    if input.trim().is_empty() {
        return Err("no YAML provided — paste a document to lint".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes ({} KiB)",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / 1024
        ));
    }

    let mut problems: Vec<Problem> = Vec::new();
    let (documents, valid) = event_pass(input, opts, &mut problems);
    line_pass(input, opts, &mut problems);

    problems.sort_by(|a, b| {
        (a.line, a.column, a.rule)
            .cmp(&(b.line, b.column, b.rule))
            .then_with(|| a.message.cmp(&b.message))
    });
    problems.dedup_by(|a, b| a == b);

    if opts.strict_warnings {
        for p in problems.iter_mut() {
            p.severity = Severity::Error;
        }
    }

    let truncated = problems.len() > MAX_PROBLEMS;
    problems.truncate(MAX_PROBLEMS);

    let errors = problems
        .iter()
        .filter(|p| p.severity == Severity::Error)
        .count();
    let warnings = problems.len() - errors;

    Ok(Report {
        valid,
        documents,
        lines: input.lines().count(),
        errors,
        warnings,
        problems,
        truncated,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub fn render(report: &Report, format: ReportFormat) -> String {
    match format {
        ReportFormat::Report => render_text(report),
        ReportFormat::Json => render_json(report),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn render_text(report: &Report) -> String {
    let docs = plural(report.documents, "document", "documents");
    let lines = plural(report.lines, "line", "lines");
    let counts = match (report.errors, report.warnings) {
        (0, 0) => String::new(),
        (e, 0) => plural(e, "error", "errors"),
        (0, w) => plural(w, "warning", "warnings"),
        (e, w) => format!(
            "{}, {}",
            plural(e, "error", "errors"),
            plural(w, "warning", "warnings")
        ),
    };

    let mut out = String::new();
    if !report.valid {
        out.push_str(&format!("✗ invalid YAML — {counts}\n"));
    } else if report.errors > 0 {
        out.push_str(&format!(
            "✗ YAML parses, but has problems — {counts} ({docs}, {lines})\n"
        ));
    } else if report.warnings > 0 {
        out.push_str(&format!("⚠ valid YAML — {counts} ({docs}, {lines})\n"));
    } else {
        out.push_str(&format!("✓ valid YAML — no problems ({docs}, {lines})\n"));
    }

    if !report.problems.is_empty() {
        out.push('\n');
        for p in &report.problems {
            out.push_str(&format!(
                "{:>4}:{:<4} {:<8} {}  [{}]\n",
                p.line,
                p.column,
                p.severity.label(),
                p.message,
                p.rule
            ));
        }
    }
    if report.truncated {
        out.push_str(&format!(
            "\n… only the first {MAX_PROBLEMS} problems are listed.\n"
        ));
    }
    out
}

fn render_json(report: &Report) -> String {
    let problems: Vec<_> = report
        .problems
        .iter()
        .map(|p| {
            json!({
                "line": p.line,
                "column": p.column,
                "level": p.severity.label(),
                "rule": p.rule,
                "message": p.message,
            })
        })
        .collect();
    let value = json!({
        "valid": report.valid,
        "documents": report.documents,
        "lines": report.lines,
        "errors": report.errors,
        "warnings": report.warnings,
        "truncated": report.truncated,
        "problems": problems,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Event pass (structure-aware rules)
// ---------------------------------------------------------------------------

struct MapFrame {
    /// key text → line it was first defined on
    keys: HashMap<String, usize>,
    expect_key: bool,
    last_key: Option<String>,
    current_key: Option<String>,
}

enum Frame {
    Map(MapFrame),
    Seq,
}

/// Walk the parser events. Returns `(documents, parsed_ok)`.
fn event_pass(input: &str, opts: &Options, out: &mut Vec<Problem>) -> (usize, bool) {
    let mut parser = Parser::new_from_str(input);
    let mut stack: Vec<Frame> = Vec::new();
    let mut documents = 0usize;

    loop {
        let (ev, mark) = match parser.next_token() {
            Ok(v) => v,
            Err(e) => {
                if opts.enabled("syntax") {
                    let m = e.marker();
                    out.push(Problem {
                        line: m.line().max(1),
                        column: m.col() + 1,
                        severity: Severity::Error,
                        rule: "syntax",
                        message: format!("{} — the document does not parse", e.info()),
                    });
                }
                return (documents.max(1), false);
            }
        };

        match ev {
            Event::StreamStart => {}
            Event::StreamEnd => return (documents.max(1), true),
            Event::DocumentStart => {
                documents += 1;
                stack.clear();
            }
            Event::DocumentEnd => stack.clear(),
            Event::Alias(_) => {
                node_start(&mut stack, None, TScalarStyle::Plain, mark, opts, out);
                node_end(&mut stack);
            }
            Event::Scalar(ref value, style, _, _) => {
                node_start(&mut stack, Some(value), style, mark, opts, out);
                node_end(&mut stack);
            }
            Event::MappingStart(..) => {
                node_start(&mut stack, None, TScalarStyle::Plain, mark, opts, out);
                stack.push(Frame::Map(MapFrame {
                    keys: HashMap::new(),
                    expect_key: true,
                    last_key: None,
                    current_key: None,
                }));
            }
            Event::MappingEnd => {
                stack.pop();
                node_end(&mut stack);
            }
            Event::SequenceStart(..) => {
                node_start(&mut stack, None, TScalarStyle::Plain, mark, opts, out);
                stack.push(Frame::Seq);
            }
            Event::SequenceEnd => {
                stack.pop();
                node_end(&mut stack);
            }
            Event::Nothing => {}
        }
    }
}

/// Called when a node begins in the parent container: this is where a scalar is
/// classified as a key or a value and the structure-aware rules fire.
fn node_start(
    stack: &mut [Frame],
    scalar: Option<&str>,
    style: TScalarStyle,
    mark: Marker,
    opts: &Options,
    out: &mut Vec<Problem>,
) {
    let line = mark.line().max(1);
    let column = mark.col() + 1;
    let plain = style == TScalarStyle::Plain;

    let in_key_position = matches!(stack.last(), Some(Frame::Map(m)) if m.expect_key);

    if in_key_position {
        let Some(Frame::Map(frame)) = stack.last_mut() else {
            return;
        };
        let Some(key) = scalar else {
            frame.current_key = None;
            return;
        };
        frame.current_key = Some(key.to_string());

        let is_merge = key == "<<";
        if opts.enabled("key-duplicates") && (!is_merge || opts.preset == Preset::Strict) {
            if let Some(first) = frame.keys.get(key) {
                out.push(Problem {
                    line,
                    column,
                    severity: Severity::Error,
                    rule: "key-duplicates",
                    message: format!(
                        "duplicate key '{key}' — first defined on line {first}; the later value silently wins"
                    ),
                });
            }
        }
        frame.keys.entry(key.to_string()).or_insert(line);

        if opts.enabled("key-ordering") {
            if let Some(prev) = &frame.last_key {
                if key.to_lowercase() < prev.to_lowercase() {
                    out.push(Problem {
                        line,
                        column,
                        severity: Severity::Warning,
                        rule: "key-ordering",
                        message: format!("key '{key}' is not in alphabetical order (after '{prev}')"),
                    });
                }
            }
        }
        frame.last_key = Some(key.to_string());

        // A key can be a truthy/octal trap too (`on: push` in a workflow file).
        if let Some(text) = scalar {
            scalar_value_rules(text, plain, line, column, opts, out);
        }
        return;
    }

    // Value position.
    let current_key = match stack.last() {
        Some(Frame::Map(m)) => m.current_key.clone(),
        _ => None,
    };

    if let Some(text) = scalar {
        if text.is_empty() && plain && opts.enabled("empty-values") {
            let key = current_key.unwrap_or_else(|| "?".to_string());
            out.push(Problem {
                line,
                column,
                severity: Severity::Warning,
                rule: "empty-values",
                message: format!("empty value for key '{key}' — it parses as null"),
            });
        } else {
            scalar_value_rules(text, plain, line, column, opts, out);
        }
    }
}

/// Rules that apply to any unquoted (plain) scalar, key or value.
fn scalar_value_rules(
    text: &str,
    plain: bool,
    line: usize,
    column: usize,
    opts: &Options,
    out: &mut Vec<Problem>,
) {
    if !plain {
        return; // quoted scalars are unambiguous by construction
    }
    if opts.enabled("truthy") && is_truthy_trap(text) {
        out.push(Problem {
            line,
            column,
            severity: Severity::Warning,
            rule: "truthy",
            message: format!(
                "truthy value '{text}' — YAML 1.1 parsers read this as a boolean; quote it or write true/false"
            ),
        });
    }
    if opts.enabled("octal-values") {
        if is_implicit_octal(text) {
            out.push(Problem {
                line,
                column,
                severity: Severity::Warning,
                rule: "octal-values",
                message: format!(
                    "'{text}' looks octal because of the leading zero — quote it or write 0o{}",
                    text.trim_start_matches('0')
                ),
            });
        } else if opts.preset == Preset::Strict && is_explicit_octal(text) {
            out.push(Problem {
                line,
                column,
                severity: Severity::Warning,
                rule: "octal-values",
                message: format!("'{text}' is an explicit octal; YAML 1.1 parsers read it as a string"),
            });
        }
    }
}

fn is_truthy_trap(text: &str) -> bool {
    if text == "true" || text == "false" {
        return false;
    }
    matches!(
        text.to_ascii_lowercase().as_str(),
        "yes" | "no" | "on" | "off" | "true" | "false"
    )
}

fn is_implicit_octal(text: &str) -> bool {
    text.len() >= 2
        && text.starts_with('0')
        && text[1..].chars().all(|c| ('0'..='7').contains(&c))
}

fn is_explicit_octal(text: &str) -> bool {
    let rest = match text.strip_prefix("0o") {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty() && rest.chars().all(|c| ('0'..='7').contains(&c))
}

/// A node finished in its parent container: flip the parent mapping's
/// key/value expectation.
fn node_end(stack: &mut [Frame]) {
    if let Some(Frame::Map(frame)) = stack.last_mut() {
        frame.expect_key = !frame.expect_key;
    }
}

// ---------------------------------------------------------------------------
// Line pass (cosmetic rules)
// ---------------------------------------------------------------------------

/// Quote-aware scan of one line: where the comment starts and how the flow
/// nesting depth changes.
struct LineScan {
    comment_at: Option<usize>,
    depth_delta: i32,
}

fn scan_line(line: &str) -> LineScan {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut comment_at = None;
    let mut depth_delta = 0i32;
    let mut prev_ws = true; // start of line counts as whitespace

    for (i, c) in line.char_indices() {
        if in_double {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_double = false;
            }
            prev_ws = false;
            continue;
        }
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            prev_ws = false;
            continue;
        }
        match c {
            '"' => in_double = true,
            '\'' => in_single = true,
            '#' if prev_ws => {
                comment_at = Some(i);
                break;
            }
            '[' | '{' => depth_delta += 1,
            ']' | '}' => depth_delta -= 1,
            _ => {}
        }
        prev_ws = c.is_whitespace();
    }
    LineScan {
        comment_at,
        depth_delta,
    }
}

/// Does this line open a block scalar (`|`, `>`, with optional chomping /
/// explicit indent indicators)?
fn opens_block_scalar(code: &str) -> bool {
    let t = code.trim_end();
    let mut chars = t.chars().rev();
    let mut seen_indicator = false;
    // Walk backwards over the optional chomping/indent indicators.
    let mut tail: Vec<char> = Vec::new();
    for c in chars.by_ref() {
        if c == '|' || c == '>' {
            seen_indicator = true;
            break;
        }
        if c == '+' || c == '-' || c.is_ascii_digit() {
            tail.push(c);
            continue;
        }
        return false;
    }
    if !seen_indicator || tail.len() > 2 {
        return false;
    }
    // `|` must be its own token: the char before it is whitespace or nothing.
    match chars.next() {
        None => true,
        Some(c) => c.is_whitespace(),
    }
}

fn char_col(line: &str, byte_idx: usize) -> usize {
    line[..byte_idx].chars().count() + 1
}

fn line_pass(input: &str, opts: &Options, out: &mut Vec<Problem>) {
    let lines: Vec<&str> = input.lines().collect();
    let mut depth = 0i32;
    let mut block_base: Option<usize> = None;
    let mut blank_run = 0usize;
    let mut seen_content = false;

    for (idx, raw) in lines.iter().enumerate() {
        let lineno = idx + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let indent_bytes = line.len() - line.trim_start().len();
        let indent_cols = line[..indent_bytes].chars().count();
        let trimmed = line.trim();
        let blank = trimmed.is_empty();

        // --- rules that apply to every line, block scalars included ----------
        if opts.enabled("line-length") && opts.max_line_length > 0 {
            let len = line.chars().count();
            if len > opts.max_line_length {
                out.push(Problem {
                    line: lineno,
                    column: opts.max_line_length + 1,
                    severity: Severity::Warning,
                    rule: "line-length",
                    message: format!(
                        "line is {len} characters long (limit {})",
                        opts.max_line_length
                    ),
                });
            }
        }

        // --- block-scalar body: skip everything structural -------------------
        let in_block_body = match block_base {
            Some(base) => {
                if blank || indent_cols > base {
                    true
                } else {
                    block_base = None;
                    false
                }
            }
            None => false,
        };

        if blank {
            if !in_block_body {
                blank_run += 1;
                if opts.enabled("empty-lines") && blank_run == 3 {
                    out.push(Problem {
                        line: lineno,
                        column: 1,
                        severity: Severity::Warning,
                        rule: "empty-lines",
                        message: "too many consecutive blank lines (limit 2)".to_string(),
                    });
                }
            }
            continue;
        }
        blank_run = 0;

        if in_block_body {
            continue;
        }

        if opts.enabled("trailing-spaces") {
            let stripped = line.trim_end_matches([' ', '\t']);
            if stripped.len() < line.len() {
                out.push(Problem {
                    line: lineno,
                    column: char_col(line, stripped.len()),
                    severity: Severity::Warning,
                    rule: "trailing-spaces",
                    message: "trailing whitespace".to_string(),
                });
            }
        }

        if opts.enabled("indentation") {
            if let Some(tab) = line[..indent_bytes].find('\t') {
                out.push(Problem {
                    line: lineno,
                    column: char_col(line, tab),
                    severity: Severity::Error,
                    rule: "indentation",
                    message: "tab character in indentation — YAML forbids tabs, indent with spaces"
                        .to_string(),
                });
            }
        }

        let scan = scan_line(line);
        let code_end = scan.comment_at.unwrap_or(line.len());
        let code = &line[..code_end];
        let code_trimmed = code.trim();
        let comment_only = code_trimmed.is_empty();

        // Comment formatting.
        if opts.enabled("comments") {
            if let Some(at) = scan.comment_at {
                let after = line[at + 1..].chars().next();
                let is_shebang = lineno == 1 && at == 0 && after == Some('!');
                if !is_shebang && matches!(after, Some(c) if c != ' ' && c != '#') {
                    out.push(Problem {
                        line: lineno,
                        column: char_col(line, at),
                        severity: Severity::Warning,
                        rule: "comments",
                        message: "missing space after '#' in comment".to_string(),
                    });
                }
                if !comment_only {
                    let before = &line[..at];
                    let spaces = before.len() - before.trim_end_matches(' ').len();
                    if spaces < 2 {
                        out.push(Problem {
                            line: lineno,
                            column: char_col(line, at),
                            severity: Severity::Warning,
                            rule: "comments",
                            message: "an inline comment needs at least 2 spaces before '#'"
                                .to_string(),
                        });
                    }
                }
            }
        }

        if !comment_only && depth == 0 && indent_cols == indent_bytes {
            // Indent width: only meaningful outside flow collections.
            if opts.enabled("indentation")
                && !line[..indent_bytes].contains('\t')
                && indent_cols % opts.indent_spaces != 0
            {
                out.push(Problem {
                    line: lineno,
                    column: indent_cols + 1,
                    severity: Severity::Warning,
                    rule: "indentation",
                    message: format!(
                        "indented {indent_cols} spaces, which is not a multiple of {}",
                        opts.indent_spaces
                    ),
                });
            }
            colon_hyphen_rules(code, line, lineno, indent_bytes, opts, out);
            empty_value_line_rule(code, line, lineno, opts, out);
        }

        if !comment_only {
            if opts.enabled("document-start") && !seen_content && code_trimmed != "---" {
                out.push(Problem {
                    line: lineno,
                    column: 1,
                    severity: Severity::Warning,
                    rule: "document-start",
                    message: "missing document start marker '---'".to_string(),
                });
            }
            seen_content = true;
            if depth == 0 && opens_block_scalar(code) {
                block_base = Some(indent_cols);
            }
            depth = (depth + scan.depth_delta).max(0);
        }
    }

    if opts.enabled("new-line-at-end-of-file") && !input.is_empty() && !input.ends_with('\n') {
        let last = lines.len().max(1);
        out.push(Problem {
            line: last,
            column: lines.last().map(|l| l.chars().count() + 1).unwrap_or(1),
            severity: Severity::Warning,
            rule: "new-line-at-end-of-file",
            message: "no newline at end of file".to_string(),
        });
    }
}

/// `colons` + `hyphens` spacing on a code (comment-stripped) line.
fn colon_hyphen_rules(
    code: &str,
    line: &str,
    lineno: usize,
    indent_bytes: usize,
    opts: &Options,
    out: &mut Vec<Problem>,
) {
    let mut cursor = indent_bytes;

    // A sequence entry: `-` then the item content.
    let rest = &code[cursor..];
    if rest.starts_with('-') && (rest.len() == 1 || rest[1..].starts_with([' ', '\t'])) {
        let after = &rest[1..];
        let spaces = after.len() - after.trim_start_matches(' ').len();
        if opts.enabled("hyphens") && spaces > 1 && !after.trim().is_empty() {
            out.push(Problem {
                line: lineno,
                column: char_col(line, cursor + 2),
                severity: Severity::Warning,
                rule: "hyphens",
                message: format!("{spaces} spaces after '-' (expected 1)"),
            });
        }
        cursor += 1 + spaces;
    }

    if !opts.enabled("colons") {
        return;
    }
    let key_part = &code[cursor..];
    // Only the first colon of a plain key is examined, so `time: 12:30` and
    // `url: http://x` can never be flagged.
    let Some(rel) = key_part.find(':') else {
        return;
    };
    let key = &key_part[..rel];
    let key_trimmed = key.trim_end();
    if key_trimmed.is_empty()
        || !key_trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
    {
        return;
    }
    let colon_byte = cursor + rel;
    let spaces_before = key.len() - key_trimmed.len();
    if spaces_before > 0 {
        out.push(Problem {
            line: lineno,
            column: char_col(line, colon_byte - spaces_before),
            severity: Severity::Warning,
            rule: "colons",
            message: format!("{spaces_before} spaces before ':' (expected none)"),
        });
    }
    let after = &code[colon_byte + 1..];
    match after.chars().next() {
        None => {}
        Some(' ') => {
            let extra = after.len() - after.trim_start_matches(' ').len();
            if extra > 1 && !after.trim().is_empty() {
                out.push(Problem {
                    line: lineno,
                    column: char_col(line, colon_byte + 2),
                    severity: Severity::Warning,
                    rule: "colons",
                    message: format!("{extra} spaces after ':' (expected 1)"),
                });
            }
        }
        // `key://…` is a URL, not a mapping.
        Some('/') => {}
        Some(_) => {
            out.push(Problem {
                line: lineno,
                column: char_col(line, colon_byte + 1),
                severity: Severity::Warning,
                rule: "colons",
                message: format!(
                    "missing space after ':' — '{key_trimmed}:…' parses as one plain scalar, not a mapping"
                ),
            });
        }
    }
}

/// yaml-rust2 does not emit an empty scalar event for every implicit-null
/// `key:` entry, so the strict empty-values rule also needs a line-level check.
fn empty_value_line_rule(
    code: &str,
    line: &str,
    lineno: usize,
    opts: &Options,
    out: &mut Vec<Problem>,
) {
    if !opts.enabled("empty-values") {
        return;
    }
    let Some(colon_byte) = code.rfind(':') else {
        return;
    };
    if !code[colon_byte + 1..].trim().is_empty() {
        return;
    }
    let key = code[..colon_byte].trim();
    if key.is_empty()
        || key.starts_with('-')
        || key.contains(['{', '}', '[', ']'])
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
    {
        return;
    }
    out.push(Problem {
        line: lineno,
        column: char_col(line, colon_byte + 1),
        severity: Severity::Warning,
        rule: "empty-values",
        message: format!("empty value for key '{key}' — it parses as null"),
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_default(src: &str) -> Report {
        lint(src, &Options::default()).expect("lints")
    }

    fn rules_of(r: &Report) -> Vec<&str> {
        r.problems.iter().map(|p| p.rule).collect()
    }

    #[test]
    fn clean_document_has_no_problems() {
        let r = lint_default("---\nserver:\n  host: localhost\n  port: 8080\n");
        assert!(r.valid);
        assert_eq!(r.problems, vec![]);
        assert_eq!(r.documents, 1);
        assert_eq!(r.lines, 4);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = lint("   \n", &Options::default()).unwrap_err();
        assert!(err.contains("no YAML provided"), "{err}");
    }

    #[test]
    fn duplicate_key_reports_both_lines() {
        let r = lint_default("name: a\nport: 1\nname: b\n");
        let dup = r
            .problems
            .iter()
            .find(|p| p.rule == "key-duplicates")
            .expect("duplicate reported");
        assert_eq!(dup.line, 3);
        assert_eq!(dup.column, 1);
        assert_eq!(dup.severity, Severity::Error);
        assert!(dup.message.contains("first defined on line 1"), "{dup:?}");
        assert!(r.valid, "the document still parses");
    }

    #[test]
    fn nested_mappings_do_not_cross_contaminate() {
        let r = lint_default("a:\n  name: x\nb:\n  name: y\n");
        assert!(!rules_of(&r).contains(&"key-duplicates"));
    }

    #[test]
    fn syntax_error_is_line_precise() {
        let r = lint_default("a: 1\n b: 2\n");
        assert!(!r.valid);
        let p = r
            .problems
            .iter()
            .find(|p| p.rule == "syntax")
            .expect("syntax error reported");
        assert_eq!(p.rule, "syntax");
        assert_eq!(p.severity, Severity::Error);
        assert_eq!(p.line, 2);
    }

    #[test]
    fn tab_indentation_is_an_error() {
        let r = lint("root:\n\tchild: 1\n", &Options::default()).unwrap();
        let tab = r
            .problems
            .iter()
            .find(|p| p.rule == "indentation" && p.message.contains("tab"))
            .expect("tab reported");
        assert_eq!((tab.line, tab.column), (2, 1));
        assert_eq!(tab.severity, Severity::Error);
    }

    #[test]
    fn odd_indent_is_flagged_against_the_step() {
        let r = lint_default("root:\n   child: 1\n");
        assert!(r
            .problems
            .iter()
            .any(|p| p.rule == "indentation" && p.message.contains("multiple of 2")));
        let four = Options {
            indent_spaces: 4,
            ..Options::default()
        };
        let r4 = lint("root:\n    child: 1\n", &four).unwrap();
        assert!(!rules_of(&r4).contains(&"indentation"));
    }

    #[test]
    fn truthy_and_octal_traps_only_fire_unquoted() {
        let r = lint_default("debug: yes\nmode: 0755\nsafe: \"yes\"\nalso: '0755'\n");
        let truthy: Vec<_> = r.problems.iter().filter(|p| p.rule == "truthy").collect();
        assert_eq!(truthy.len(), 1);
        assert_eq!(truthy[0].line, 1);
        let octal: Vec<_> = r
            .problems
            .iter()
            .filter(|p| p.rule == "octal-values")
            .collect();
        assert_eq!(octal.len(), 1);
        assert_eq!(octal[0].line, 2);
    }

    #[test]
    fn colon_and_hyphen_spacing() {
        let r = lint_default("key:value\nother :  1\nlist:\n  -   a\n");
        let msgs: Vec<&str> = r.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("missing space after ':'")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("spaces before ':'")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("spaces after ':'")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("after '-'")), "{msgs:?}");
    }

    #[test]
    fn urls_and_times_are_not_colon_problems() {
        let r = lint_default("url: http://example.com/a\nstart: 12:30\nratio: 3:4\n");
        assert!(!rules_of(&r).contains(&"colons"), "{:?}", r.problems);
    }

    #[test]
    fn block_scalars_are_not_linted_as_yaml() {
        let r = lint_default("script: |\n  if [ x ]; then\n    echo yes:no\n  fi\nname: ok\n");
        assert!(!rules_of(&r).contains(&"colons"), "{:?}", r.problems);
        assert!(!rules_of(&r).contains(&"truthy"), "{:?}", r.problems);
    }

    #[test]
    fn comments_and_trailing_space_and_length() {
        let opts = Options {
            max_line_length: 20,
            ..Options::default()
        };
        let r = lint("a: 1 # tight\nb: 2  #no space\nc: 3   \nd: aaaaaaaaaaaaaaaaaaaaaaaa\n", &opts)
            .unwrap();
        let msgs: Vec<&str> = r.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("at least 2 spaces")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("missing space after '#'")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("trailing whitespace")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("limit 20")), "{msgs:?}");
    }

    #[test]
    fn newline_and_blank_line_rules() {
        let r = lint_default("a: 1\n\n\n\nb: 2");
        let msgs: Vec<&str> = r.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("consecutive blank lines")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("no newline at end of file")), "{msgs:?}");
    }

    #[test]
    fn relaxed_drops_cosmetics_but_keeps_real_problems() {
        let src = "name: a\nname: b   \ndebug: yes\n";
        let relaxed = lint(
            src,
            &Options {
                preset: Preset::Relaxed,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(rules_of(&relaxed), vec!["key-duplicates"]);
        assert_eq!(relaxed.errors, 1);
        assert_eq!(relaxed.warnings, 0);
    }

    #[test]
    fn strict_adds_document_start_key_order_and_empty_values() {
        let src = "zeta: 1\nalpha: 2\nempty:\n";
        let strict = lint(
            src,
            &Options {
                preset: Preset::Strict,
                ..Options::default()
            },
        )
        .unwrap();
        let rules = rules_of(&strict);
        assert!(rules.contains(&"document-start"), "{rules:?}");
        assert!(rules.contains(&"key-ordering"), "{rules:?}");
        assert!(rules.contains(&"empty-values"), "{rules:?}");
        // …and none of them fire under the default preset.
        let default = lint_default(src);
        for r in ["document-start", "key-ordering", "empty-values"] {
            assert!(!rules_of(&default).contains(&r), "{r} should be strict-only");
        }
    }

    #[test]
    fn disabling_a_rule_silences_it() {
        let opts = Options {
            disabled: parse_rule_list("truthy, line-length").unwrap(),
            ..Options::default()
        };
        let r = lint("debug: yes\n", &opts).unwrap();
        assert_eq!(r.problems, vec![]);
    }

    #[test]
    fn unknown_rule_id_is_rejected() {
        let err = parse_rule_list("truthy,not-a-rule").unwrap_err();
        assert!(err.contains("unknown rule 'not-a-rule'"), "{err}");
        assert!(err.contains("key-duplicates"), "{err}");
    }

    #[test]
    fn strict_warnings_promotes_every_warning() {
        let opts = Options {
            strict_warnings: true,
            ..Options::default()
        };
        let r = lint("debug: yes\n", &opts).unwrap();
        assert_eq!(r.errors, 1);
        assert_eq!(r.warnings, 0);
        assert!(r.valid);
    }

    #[test]
    fn multi_document_streams_are_counted() {
        let r = lint_default("---\na: 1\n---\nb: 2\n");
        assert_eq!(r.documents, 2);
        assert!(r.valid);
    }

    #[test]
    fn merge_keys_are_only_duplicates_under_strict() {
        let src = "base: &b\n  a: 1\nmix:\n  <<: *b\n  <<: *b\n  c: 2\n";
        assert!(!rules_of(&lint_default(src)).contains(&"key-duplicates"));
        let strict = lint(
            src,
            &Options {
                preset: Preset::Strict,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(rules_of(&strict).contains(&"key-duplicates"));
    }

    #[test]
    fn flow_collections_do_not_trip_indentation() {
        let r = lint_default("list: [\n   a,\n   b\n]\n");
        assert!(!rules_of(&r).contains(&"indentation"), "{:?}", r.problems);
    }

    #[test]
    fn text_report_shape() {
        let out = run("name: a\nname: b\n", "relaxed", 2, 80, "", false, "report").unwrap();
        assert_eq!(
            out,
            "✗ YAML parses, but has problems — 1 error (1 document, 2 lines)\n\n   2:1    error    duplicate key 'name' — first defined on line 1; the later value silently wins  [key-duplicates]\n"
        );
    }

    #[test]
    fn clean_report_shape() {
        let out = run("---\na: 1\n", "default", 2, 80, "", false, "report").unwrap();
        assert_eq!(out, "✓ valid YAML — no problems (1 document, 2 lines)\n");
    }

    #[test]
    fn json_report_is_machine_readable() {
        let out = run("debug: yes\n", "default", 2, 80, "", false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["warnings"], 1);
        assert_eq!(v["errors"], 0);
        assert_eq!(v["problems"][0]["rule"], "truthy");
        assert_eq!(v["problems"][0]["line"], 1);
        assert_eq!(v["problems"][0]["level"], "warning");
    }

    #[test]
    fn string_entry_point_matches_typed_one() {
        let a = run("debug: yes\n", "default", 2, 80, "", true, "json").unwrap();
        let b = run_str("debug: yes\n", "default", "2", "80", "", "true", "json").unwrap();
        assert_eq!(a, b);
        assert!(run_str("a: 1\n", "default", "", "", "", "false", "").is_ok());
    }

    #[test]
    fn bad_arguments_are_rejected() {
        assert!(run("a: 1\n", "loose", 2, 80, "", false, "report")
            .unwrap_err()
            .contains("unknown preset"));
        assert!(run("a: 1\n", "default", 0, 80, "", false, "report")
            .unwrap_err()
            .contains("indent_spaces"));
        assert!(run("a: 1\n", "default", 2, 80, "", false, "yolo")
            .unwrap_err()
            .contains("unknown report_format"));
        assert!(run_str("a: 1\n", "default", "two", "80", "", "false", "report")
            .unwrap_err()
            .contains("whole number"));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = format!("a: {}\n", "x".repeat(MAX_INPUT_BYTES));
        let err = lint(&big, &Options::default()).unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }

    #[test]
    fn line_length_can_be_disabled_with_zero() {
        let opts = Options {
            max_line_length: 0,
            ..Options::default()
        };
        let r = lint(&format!("a: {}\n", "x".repeat(300)), &opts).unwrap();
        assert!(!rules_of(&r).contains(&"line-length"));
    }
}
