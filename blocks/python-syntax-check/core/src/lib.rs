//! python-syntax-check core — compile-time syntax checking for Python 3 source,
//! shared by the chat skill block, the CLI and the web page.
//!
//! The engine is the `rustpython-parser` grammar (the same family of messages
//! CPython itself raises at compile time). Source is parsed, never executed, so
//! runtime errors such as `NameError` are out of scope by construction — this
//! mirrors `python -m py_compile`, not `python`.

use rustpython_parser::{lexer::LexicalErrorType, parse, Mode, ParseErrorType, Tok};

/// Largest accepted source, in bytes. A pasted module far past this is almost
/// always a mis-paste, and the parser is O(n) but not free.
pub const MAX_SOURCE_BYTES: usize = 200_000;

/// One compile-time diagnostic, already resolved to 1-based line/column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// `SyntaxError`, `IndentationError` or `TabError` — the three classes
    /// CPython raises while compiling.
    pub kind: &'static str,
    pub message: String,
    pub line: usize,
    /// 1-based, counted in characters (not bytes) from the start of the line.
    pub column: usize,
    /// Byte offset into the (newline-normalized) source.
    pub offset: usize,
    /// The offending source line, verbatim.
    pub source_line: String,
}

/// A recognised Python-2-only construct and its Python 3 replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Python2Hint {
    pub line: usize,
    pub construct: &'static str,
    pub message: &'static str,
}

/// Input size summary, mirroring what a code editor's status bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub lines: usize,
    pub non_empty_lines: usize,
    pub characters: usize,
}

/// Full result of a check: at most one parse error (the parser stops at the
/// first, exactly like CPython) plus any advisory Python-2 hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub filename: String,
    pub mode: String,
    pub error: Option<Diagnostic>,
    pub hints: Vec<Python2Hint>,
    pub stats: Stats,
}

impl Report {
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
}

/// Convenience entry point used by the scaffolded `run` export: check `code` as
/// a module and render the default text report.
pub fn run(code: &str) -> Result<String, String> {
    run_with_options(code, "module", "text", true, true, true, "<input>")
}

/// Check `code` and render a report.
///
/// * `mode` — `module` (a whole file), `expression` (a single expression, as
///   `eval()` accepts) or `interactive` (a REPL-style block).
/// * `format` — `text` or `json`.
/// * `show_context` — echo the offending source line with a `^` caret.
/// * `python2_hints` — explain Python-2-only constructs in Python 3 terms.
/// * `stats` — include the line/character counts.
/// * `filename` — label used in the report; defaults to `<input>`.
#[allow(clippy::too_many_arguments)]
pub fn run_with_options(
    code: &str,
    mode: &str,
    format: &str,
    show_context: bool,
    python2_hints: bool,
    stats: bool,
    filename: &str,
) -> Result<String, String> {
    let report = check(code, mode, python2_hints, filename)?;
    match norm(format, "text").as_str() {
        "text" => Ok(render_text(&report, show_context, stats)),
        "json" => Ok(render_json(&report, show_context, stats)),
        other => Err(format!(
            "invalid format {other:?}: expected \"text\" or \"json\""
        )),
    }
}

/// Parse `code` and build the structured report. Returns `Err` only for bad
/// input to the tool itself (empty/over-size source, unknown mode) — a Python
/// syntax error is a normal, successful result.
pub fn check(
    code: &str,
    mode: &str,
    python2_hints: bool,
    filename: &str,
) -> Result<Report, String> {
    if code.trim().is_empty() {
        return Err("code is required: paste some Python source to check".into());
    }
    if code.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "code is too large ({} bytes); limit is {MAX_SOURCE_BYTES} bytes",
            code.len()
        ));
    }
    let mode_name = norm(mode, "module");
    let parse_mode = match mode_name.as_str() {
        "module" => Mode::Module,
        "expression" => Mode::Expression,
        "interactive" => Mode::Interactive,
        other => {
            return Err(format!(
                "invalid mode {other:?}: expected \"module\", \"expression\" or \"interactive\""
            ))
        }
    };

    let source = normalize(code);
    let label = {
        let f = filename.trim();
        if f.is_empty() {
            "<input>".to_string()
        } else {
            f.to_string()
        }
    };

    let error = match parse(&source, parse_mode, &label) {
        Ok(_) => None,
        Err(e) => {
            let offset = usize::from(e.offset).min(source.len());
            let (line, column) = line_col(&source, offset);
            Some(Diagnostic {
                kind: classify(&e.error),
                message: e.error.to_string(),
                line,
                column,
                offset,
                source_line: nth_line(&source, line).to_string(),
            })
        }
    };

    let hints = if python2_hints {
        scan_python2(&source)
    } else {
        Vec::new()
    };

    Ok(Report {
        filename: label,
        mode: mode_name,
        error,
        hints,
        stats: stats_of(&source),
    })
}

// ---------------------------------------------------------------- engine glue

/// Map a parser error onto the CPython exception class a real interpreter would
/// raise for it.
fn classify(err: &ParseErrorType) -> &'static str {
    match err {
        ParseErrorType::Lexical(LexicalErrorType::TabError)
        | ParseErrorType::Lexical(LexicalErrorType::TabsAfterSpaces) => "TabError",
        ParseErrorType::Lexical(LexicalErrorType::IndentationError) => "IndentationError",
        ParseErrorType::UnrecognizedToken(tok, expected) => {
            if matches!(tok, Tok::Indent | Tok::Dedent) || expected.as_deref() == Some("Indent") {
                "IndentationError"
            } else {
                "SyntaxError"
            }
        }
        _ => "SyntaxError",
    }
}

/// Strip a UTF-8 BOM and fold CRLF/CR line endings to LF, so a file pasted from
/// Windows doesn't fail on its line endings and so offsets map to what the user
/// sees.
fn normalize(code: &str) -> String {
    let body = code.strip_prefix('\u{feff}').unwrap_or(code);
    if !body.contains('\r') {
        return body.to_string();
    }
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// 1-based line and character column for a byte offset.
fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let head = &source[..offset];
    let line = head.matches('\n').count() + 1;
    let line_start = head.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = source[line_start..offset].chars().count() + 1;
    (line, column)
}

/// The `n`th (1-based) line of `source`, without its terminator.
fn nth_line(source: &str, n: usize) -> &str {
    source.split('\n').nth(n.saturating_sub(1)).unwrap_or("")
}

/// A caret-alignment prefix for `line`: every character before `column` becomes
/// a space, except tabs, which stay tabs so the caret lands under the token in a
/// monospace renderer.
fn caret_prefix(line: &str, column: usize) -> String {
    line.chars()
        .take(column.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect()
}

fn stats_of(source: &str) -> Stats {
    let body = source.strip_suffix('\n').unwrap_or(source);
    let lines: Vec<&str> = if body.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };
    Stats {
        lines: lines.len(),
        non_empty_lines: lines.iter().filter(|l| !l.trim().is_empty()).count(),
        characters: source.chars().count(),
    }
}

fn norm(value: &str, default: &str) -> String {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

// ------------------------------------------------------------ python-2 hints

/// Recognise the Python-2-only constructs whose Python 3 parse error is
/// cryptic, and say what to write instead. Comment-only lines are skipped; the
/// scan is deliberately line-based and conservative (it would rather miss a
/// construct than mislabel valid Python 3).
fn scan_python2(source: &str) -> Vec<Python2Hint> {
    let mut hints = Vec::new();
    for (idx, raw) in source.split('\n').enumerate() {
        let line = strip_line_comment(raw);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let no = idx + 1;

        if is_py2_print(trimmed) {
            hints.push(Python2Hint {
                line: no,
                construct: "print statement",
                message:
                    "`print` is a function in Python 3 — write print(\"text\") with parentheses.",
            });
        }
        if is_py2_except(trimmed) {
            hints.push(Python2Hint {
                line: no,
                construct: "except comma syntax",
                message:
                    "Python 3 binds the exception with `as`: write `except ValueError as err:`.",
            });
        }
        if is_py2_raise(trimmed) {
            hints.push(Python2Hint {
                line: no,
                construct: "raise comma syntax",
                message: "Python 3 raises an instance: write `raise ValueError(\"message\")`.",
            });
        }
        if is_py2_exec(trimmed) {
            hints.push(Python2Hint {
                line: no,
                construct: "exec statement",
                message:
                    "`exec` is a function in Python 3 — write exec(\"code\") with parentheses.",
            });
        }
        if outside_strings(line).contains("<>") {
            hints.push(Python2Hint {
                line: no,
                construct: "<> operator",
                message: "The `<>` inequality operator was removed — use `!=`.",
            });
        }
        if outside_strings(line).contains('`') {
            hints.push(Python2Hint {
                line: no,
                construct: "backtick repr",
                message: "Backtick repr was removed — use repr(value) instead of `value`.",
            });
        }
        if has_py2_octal(&outside_strings(line)) {
            hints.push(Python2Hint {
                line: no,
                construct: "legacy octal literal",
                message: "Python 3 octal literals need an `0o` prefix — write 0o755, not 0755.",
            });
        }
    }
    hints
}

fn is_py2_print(trimmed: &str) -> bool {
    py2_statement_arg(trimmed, "print").is_some_and(|rest| {
        let c = rest.chars().next().unwrap_or(' ');
        c != '(' && c != '=' && c != '>'
    })
}

fn is_py2_exec(trimmed: &str) -> bool {
    py2_statement_arg(trimmed, "exec")
        .is_some_and(|rest| matches!(rest.chars().next(), Some('"') | Some('\'')))
}

/// `<keyword> <something>` with real whitespace between them, and nothing that
/// makes the keyword part of a larger name.
fn py2_statement_arg<'a>(trimmed: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = trimmed.strip_prefix(keyword)?;
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        None
    } else {
        Some(rest)
    }
}

/// `except SomeError, name:` — the Python 2 binding form.
fn is_py2_except(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("except ") else {
        return false;
    };
    let Some(head) = rest.strip_suffix(':') else {
        return false;
    };
    let head = outside_strings(head);
    // A tuple of exception types — `except (A, B):` — is valid Python 3.
    if head.trim_start().starts_with('(') {
        return false;
    }
    let Some((_, tail)) = head.split_once(',') else {
        return false;
    };
    let tail = tail.trim();
    !tail.is_empty()
        && !tail.contains(',')
        && tail.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !tail.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// `raise SomeError, "message"` — the Python 2 form.
fn is_py2_raise(trimmed: &str) -> bool {
    let Some(rest) = py2_statement_arg(trimmed, "raise") else {
        return false;
    };
    let rest = outside_strings(rest);
    let Some((head, tail)) = rest.split_once(',') else {
        return false;
    };
    let original_tail_has_value = py2_statement_arg(trimmed, "raise")
        .and_then(|raw| raw.split_once(',').map(|(_, tail)| !tail.trim().is_empty()))
        .unwrap_or(false);
    let head = head.trim();
    !head.is_empty()
        && (!tail.trim().is_empty() || original_tail_has_value)
        && head
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !head.contains('(')
}

/// Whether the line holds a bare `0755`-style octal literal (legal in Python 2,
/// a syntax error in 3).
fn has_py2_octal(line: &str) -> bool {
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let prev_is_word =
            i > 0 && (bytes[i - 1].is_alphanumeric() || bytes[i - 1] == '_' || bytes[i - 1] == '.');
        if c == '0' && !prev_is_word {
            let mut j = i + 1;
            let mut digits = 0;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                digits += 1;
                j += 1;
            }
            let terminated = j >= bytes.len()
                || !(bytes[j].is_alphanumeric() || bytes[j] == '_' || bytes[j] == '.');
            if digits > 0 && terminated && bytes[i + 1..j].iter().all(|d| ('0'..='7').contains(d)) {
                return true;
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    false
}

/// Drop a trailing `# comment` that starts outside a string literal.
fn strip_line_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                '#' => return &line[..i],
                _ => {}
            },
        }
    }
    line
}

/// The line with the contents of every string literal blanked out, so operator
/// scans don't trip over text inside quotes.
fn outside_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                out.push(' ');
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    out.push(' ');
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------- rendering

fn render_text(report: &Report, show_context: bool, stats: bool) -> String {
    let mut out = String::new();
    match &report.error {
        None => {
            out.push_str(&format!(
                "{}: OK — no syntax errors found (Python 3 grammar, {} mode)\n",
                report.filename, report.mode
            ));
        }
        Some(d) => {
            out.push_str(&format!(
                "{}:{}:{}: {}: {}\n",
                report.filename, d.line, d.column, d.kind, d.message
            ));
            if show_context && !d.source_line.is_empty() {
                let gutter = d.line.to_string();
                let pad = " ".repeat(gutter.len());
                out.push('\n');
                out.push_str(&format!("  {gutter} | {}\n", d.source_line));
                out.push_str(&format!(
                    "  {pad} | {}^\n",
                    caret_prefix(&d.source_line, d.column)
                ));
            }
        }
    }

    if !report.hints.is_empty() {
        out.push_str("\nPython 2 constructs found\n");
        for h in &report.hints {
            out.push_str(&format!("  line {}: {}\n", h.line, h.message));
        }
    }

    if stats {
        out.push_str(&format!(
            "\nStats\n  lines: {}\n  non-empty lines: {}\n  characters: {}\n",
            report.stats.lines, report.stats.non_empty_lines, report.stats.characters
        ));
    }
    out
}

fn render_json(report: &Report, show_context: bool, stats: bool) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"filename\": {},\n",
        json_str(&report.filename)
    ));
    out.push_str(&format!("  \"mode\": {},\n", json_str(&report.mode)));
    out.push_str(&format!("  \"valid\": {},\n", report.is_valid()));
    match &report.error {
        None => out.push_str("  \"error\": null"),
        Some(d) => {
            out.push_str("  \"error\": {\n");
            out.push_str(&format!("    \"type\": {},\n", json_str(d.kind)));
            out.push_str(&format!("    \"message\": {},\n", json_str(&d.message)));
            out.push_str(&format!("    \"line\": {},\n", d.line));
            out.push_str(&format!("    \"column\": {},\n", d.column));
            out.push_str(&format!("    \"offset\": {}", d.offset));
            if show_context {
                out.push_str(&format!(
                    ",\n    \"text\": {},\n    \"caret\": {}",
                    json_str(&d.source_line),
                    json_str(&format!("{}^", caret_prefix(&d.source_line, d.column)))
                ));
            }
            out.push_str("\n  }");
        }
    }
    out.push_str(",\n  \"python2_hints\": ");
    if report.hints.is_empty() {
        out.push_str("[]");
    } else {
        out.push_str("[\n");
        for (i, h) in report.hints.iter().enumerate() {
            out.push_str(&format!(
                "    {{ \"line\": {}, \"construct\": {}, \"message\": {} }}{}\n",
                h.line,
                json_str(h.construct),
                json_str(h.message),
                if i + 1 == report.hints.len() { "" } else { "," }
            ));
        }
        out.push_str("  ]");
    }
    if stats {
        out.push_str(&format!(
            ",\n  \"stats\": {{ \"lines\": {}, \"non_empty_lines\": {}, \"characters\": {} }}",
            report.stats.lines, report.stats.non_empty_lines, report.stats.characters
        ));
    }
    out.push_str("\n}\n");
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(code: &str) -> Report {
        check(code, "module", true, "<input>").unwrap()
    }

    #[test]
    fn valid_module_reports_ok() {
        let out = run("def greet(name):\n    return f\"hi {name}\"\n").unwrap();
        assert!(
            out.starts_with("<input>: OK — no syntax errors found (Python 3 grammar, module mode)")
        );
        assert!(out.contains("lines: 2"));
    }

    #[test]
    fn missing_colon_is_a_syntax_error_with_line_and_column() {
        let r = report("def f(:\n    pass\n");
        let d = r.error.expect("expected a syntax error");
        assert_eq!(d.kind, "SyntaxError");
        assert_eq!((d.line, d.column), (1, 7));
        assert_eq!(d.source_line, "def f(:");
    }

    #[test]
    fn unexpected_indent_is_an_indentation_error() {
        let d = report("  x = 1\n").error.expect("expected an error");
        assert_eq!(d.kind, "IndentationError");
        assert_eq!(d.line, 1);
    }

    #[test]
    fn missing_indented_block_is_an_indentation_error() {
        let d = report("if True:\nprint(1)\n")
            .error
            .expect("expected an error");
        assert_eq!(d.kind, "IndentationError");
        assert_eq!(d.message, "expected an indented block");
        assert_eq!(d.line, 2);
    }

    #[test]
    fn mixed_tabs_and_spaces_is_a_tab_error() {
        let d = report("def f():\n\tx = 1\n        y = 2\n")
            .error
            .expect("expected an error");
        assert_eq!(d.kind, "TabError");
        assert!(d.message.contains("tabs and spaces"));
    }

    #[test]
    fn unterminated_string_reports_the_line() {
        let d = report("s = 'abc\n").error.expect("expected an error");
        assert_eq!(d.kind, "SyntaxError");
        assert!(d.message.contains("EOL while scanning string literal"));
        assert_eq!(d.line, 2);
    }

    #[test]
    fn unclosed_bracket_reports_eof() {
        let d = report("x = (1,\n").error.expect("expected an error");
        assert!(d.message.contains("EOF"));
    }

    #[test]
    fn python2_print_gets_a_hint() {
        let r = report("print \"hello\"\n");
        assert!(r.error.is_some());
        assert_eq!(r.hints.len(), 1);
        assert_eq!(r.hints[0].construct, "print statement");
        assert_eq!(r.hints[0].line, 1);
    }

    #[test]
    fn python2_except_and_raise_get_hints() {
        let r = report("try:\n    pass\nexcept ValueError, err:\n    raise ValueError, \"bad\"\n");
        let kinds: Vec<&str> = r.hints.iter().map(|h| h.construct).collect();
        assert!(kinds.contains(&"except comma syntax"), "{kinds:?}");
        assert!(kinds.contains(&"raise comma syntax"), "{kinds:?}");
    }

    #[test]
    fn python2_operators_get_hints() {
        let r = report("if a <> b:\n    c = `a`\n    m = 0755\n");
        let kinds: Vec<&str> = r.hints.iter().map(|h| h.construct).collect();
        assert!(kinds.contains(&"<> operator"), "{kinds:?}");
        assert!(kinds.contains(&"backtick repr"), "{kinds:?}");
        assert!(kinds.contains(&"legacy octal literal"), "{kinds:?}");
    }

    #[test]
    fn valid_python3_does_not_get_python2_hints() {
        let r = report(
            "print(\"ok\")\nx = 0o755\ny = 0\nz = 0.5\ntry:\n    pass\nexcept (ValueError, TypeError):\n    raise RuntimeError(\"no\")\n",
        );
        assert!(r.is_valid());
        assert!(r.hints.is_empty(), "{:?}", r.hints);
    }

    #[test]
    fn strings_and_comments_do_not_trigger_hints() {
        let r = report("msg = \"use print \\\"x\\\" in py2\"  # print \"x\" and `a` and 0755\n");
        assert!(r.is_valid());
        assert!(r.hints.is_empty(), "{:?}", r.hints);
    }

    #[test]
    fn hints_can_be_disabled() {
        let r = check("print \"hi\"\n", "module", false, "<input>").unwrap();
        assert!(r.hints.is_empty());
    }

    #[test]
    fn expression_mode_rejects_a_statement() {
        let r = check("x = 1", "expression", true, "<input>").unwrap();
        assert!(r.error.is_some());
        let ok = check("1 + 2 * 3", "expression", true, "<input>").unwrap();
        assert!(ok.is_valid());
    }

    #[test]
    fn interactive_mode_accepts_a_repl_block() {
        let r = check("x = 1\n", "interactive", true, "<input>").unwrap();
        assert!(r.is_valid(), "{:?}", r.error);
    }

    #[test]
    fn caret_context_is_aligned_and_optional() {
        let with =
            run_with_options("def f(:\n", "module", "text", true, true, false, "app.py").unwrap();
        assert!(with.contains("app.py:1:7: SyntaxError:"), "{with}");
        assert!(with.contains("  1 | def f(:\n"), "{with}");
        assert!(with.contains("    |       ^\n"), "{with}");
        let without =
            run_with_options("def f(:\n", "module", "text", false, true, false, "app.py").unwrap();
        assert!(!without.contains('^'), "{without}");
    }

    #[test]
    fn tab_indented_context_keeps_tabs_in_the_caret_prefix() {
        let d = report("def f():\n\tx = (\n")
            .error
            .expect("expected an error");
        let prefix = caret_prefix(&d.source_line, d.column);
        assert_eq!(prefix, "");
    }

    #[test]
    fn json_format_has_the_documented_shape() {
        let out =
            run_with_options("def f(:\n", "module", "json", true, true, true, "app.py").unwrap();
        assert!(out.contains("\"valid\": false"));
        assert!(out.contains("\"type\": \"SyntaxError\""));
        assert!(out.contains("\"line\": 1"));
        assert!(out.contains("\"column\": 7"));
        assert!(out.contains("\"text\": \"def f(:\""));
        assert!(out.contains("\"caret\": \"      ^\""));
        assert!(out.contains("\"stats\""));
    }

    #[test]
    fn json_valid_source_has_null_error() {
        let out = run_with_options("x = 1\n", "module", "json", true, false, false, "").unwrap();
        assert!(out.contains("\"valid\": true"));
        assert!(out.contains("\"error\": null"));
        assert!(out.contains("\"filename\": \"<input>\""));
        assert!(!out.contains("\"stats\""));
    }

    #[test]
    fn stats_count_lines_and_characters() {
        let s = stats_of("a = 1\n\nb = 2\n");
        assert_eq!((s.lines, s.non_empty_lines, s.characters), (3, 2, 13));
    }

    #[test]
    fn crlf_and_bom_are_normalized() {
        let r = report("\u{feff}if True:\r\n    x = 1\r\n");
        assert!(r.is_valid(), "{:?}", r.error);
        assert_eq!(r.stats.lines, 2);
    }

    #[test]
    fn unicode_columns_are_counted_in_characters() {
        let d = report("é = ((1)\n").error.expect("expected an error");
        assert_eq!(d.line, 2);
        assert_eq!(d.column, 1);
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(run("   \n").unwrap_err().contains("code is required"));
    }

    #[test]
    fn unknown_mode_and_format_are_errors() {
        assert!(check("x = 1", "script", true, "<input>")
            .unwrap_err()
            .contains("invalid mode"));
        assert!(
            run_with_options("x = 1", "module", "yaml", true, true, true, "<input>")
                .unwrap_err()
                .contains("invalid format")
        );
    }

    #[test]
    fn over_size_input_is_rejected() {
        let big = "x = 1\n".repeat(MAX_SOURCE_BYTES);
        assert!(run(&big).unwrap_err().contains("too large"));
    }
}
