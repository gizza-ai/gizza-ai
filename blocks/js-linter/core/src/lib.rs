//! js-linter core — dependency-free JavaScript lint heuristics shared by chat and web.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
struct Issue {
    line: usize,
    col: usize,
    severity: Severity,
    rule: &'static str,
    message: String,
    source: String,
}

#[derive(Debug, Clone)]
struct Options {
    preset: String,
    ecma: String,
    env: String,
    source_type: String,
    min_severity: String,
    ignore: Vec<String>,
    format: String,
}

impl Options {
    fn new(
        preset: &str,
        ecma: &str,
        env: &str,
        source_type: &str,
        min_severity: &str,
        ignore: &str,
        format: &str,
    ) -> Result<Self, String> {
        let opt = Self {
            preset: norm(preset, "recommended"),
            ecma: norm(ecma, "latest"),
            env: norm(env, "browser"),
            source_type: norm(source_type, "auto"),
            min_severity: norm(min_severity, "all"),
            ignore: ignore
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_ascii_uppercase())
                .collect(),
            format: norm(format, "text"),
        };
        one_of(&opt.preset, &["minimal", "recommended", "strict"], "preset")?;
        one_of(&opt.ecma, &["es5", "es2015", "es2020", "latest"], "ecma")?;
        one_of(&opt.env, &["browser", "node", "both", "none"], "env")?;
        one_of(
            &opt.source_type,
            &["auto", "script", "module"],
            "source_type",
        )?;
        one_of(
            &opt.min_severity,
            &["all", "warning", "error"],
            "min_severity",
        )?;
        one_of(&opt.format, &["text", "json"], "format")?;
        Ok(opt)
    }

    fn enabled(&self, rule: &str) -> bool {
        !self.ignore.iter().any(|r| r == rule)
            && match self.preset.as_str() {
                "minimal" => matches!(rule, "SYNTAX" | "EQEQ" | "UNREACHABLE" | "UNDEF-VAR"),
                "strict" | "recommended" => true,
                _ => true,
            }
    }

    fn severity_floor(&self) -> Severity {
        match self.min_severity.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        }
    }
}

fn norm(value: &str, default: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        default.to_string()
    } else {
        v.to_ascii_lowercase()
    }
}

fn one_of(value: &str, allowed: &[&str], name: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("invalid {name}: {value}"))
    }
}

/// Backward-compatible scaffold entry point.
pub fn run(input: &str) -> Result<String, String> {
    run_with_options(
        input,
        "recommended",
        "latest",
        "browser",
        "auto",
        "all",
        "",
        "text",
    )
}

/// Lint JavaScript source and return either a text report or JSON diagnostics.
pub fn run_with_options(
    code: &str,
    preset: &str,
    ecma: &str,
    env: &str,
    source_type: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("code is required".into());
    }
    if code.len() > 200_000 {
        return Err("code is too large; limit is 200000 bytes".into());
    }
    let options = Options::new(preset, ecma, env, source_type, min_severity, ignore, format)?;
    let mut issues = lint(code, &options);
    let floor = options.severity_floor();
    issues.retain(|i| i.severity >= floor && !options.ignore.iter().any(|r| r == i.rule));
    issues.sort_by_key(|i| (i.line, i.col, i.rule));
    if options.format == "json" {
        Ok(format_json(&issues))
    } else {
        Ok(format_text(&issues))
    }
}

fn lint(code: &str, options: &Options) -> Vec<Issue> {
    let mut issues = Vec::new();
    let stripped = strip_comments_and_strings(code);
    syntax_checks(code, &stripped, options, &mut issues);
    line_checks(code, &stripped, options, &mut issues);
    variable_checks(code, &stripped, options, &mut issues);
    issues
}

fn push_issue(
    issues: &mut Vec<Issue>,
    options: &Options,
    line: usize,
    col: usize,
    severity: Severity,
    rule: &'static str,
    msg: impl Into<String>,
    source: &str,
) {
    if options.enabled(rule) {
        issues.push(Issue {
            line,
            col,
            severity,
            rule,
            message: msg.into(),
            source: source.trim().to_string(),
        });
    }
}

fn strip_comments_and_strings(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut chars = code.chars().peekable();
    let mut mode: Option<char> = None;
    while let Some(c) = chars.next() {
        match mode {
            Some('\'') | Some('"') | Some('`') => {
                let quote = mode.unwrap();
                if c == '\\' {
                    out.push(' ');
                    if let Some(n) = chars.next() {
                        out.push(if n == '\n' { '\n' } else { ' ' });
                    }
                } else if c == quote {
                    out.push(' ');
                    mode = None;
                } else {
                    out.push(if c == '\n' { '\n' } else { ' ' });
                }
            }
            Some('*') => {
                if c == '*' && chars.peek() == Some(&'/') {
                    out.push(' ');
                    chars.next();
                    out.push(' ');
                    mode = None;
                } else {
                    out.push(if c == '\n' { '\n' } else { ' ' });
                }
            }
            _ => {
                if c == '/' && chars.peek() == Some(&'/') {
                    out.push(' ');
                    chars.next();
                    out.push(' ');
                    while let Some(n) = chars.next() {
                        out.push(if n == '\n' { '\n' } else { ' ' });
                        if n == '\n' {
                            break;
                        }
                    }
                } else if c == '/' && chars.peek() == Some(&'*') {
                    out.push(' ');
                    chars.next();
                    out.push(' ');
                    mode = Some('*');
                } else if matches!(c, '\'' | '"' | '`') {
                    out.push(' ');
                    mode = Some(c);
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

fn syntax_checks(code: &str, stripped: &str, options: &Options, issues: &mut Vec<Issue>) {
    let mut stack: Vec<(char, usize, usize)> = Vec::new();
    for (line_idx, line) in stripped.lines().enumerate() {
        for (col_idx, ch) in line.chars().enumerate() {
            match ch {
                '(' | '[' | '{' => stack.push((ch, line_idx + 1, col_idx + 1)),
                ')' | ']' | '}' => {
                    let want = match ch {
                        ')' => '(',
                        ']' => '[',
                        '}' => '{',
                        _ => unreachable!(),
                    };
                    if !matches!(stack.pop(), Some((open, _, _)) if open == want) {
                        push_issue(
                            issues,
                            options,
                            line_idx + 1,
                            col_idx + 1,
                            Severity::Error,
                            "SYNTAX",
                            format!("unmatched `{ch}`"),
                            line,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    for (open, line, col) in stack {
        let src = code.lines().nth(line.saturating_sub(1)).unwrap_or("");
        push_issue(
            issues,
            options,
            line,
            col,
            Severity::Error,
            "SYNTAX",
            format!("unclosed `{open}`"),
            src,
        );
    }
    if code.matches("/*").count() > code.matches("*/").count() {
        push_issue(
            issues,
            options,
            code.lines().count().max(1),
            1,
            Severity::Error,
            "SYNTAX",
            "unterminated block comment",
            "",
        );
    }
    for (idx, line) in code.lines().enumerate() {
        if has_unterminated_quote(line, '\'')
            || has_unterminated_quote(line, '"')
            || has_unterminated_quote(line, '`')
        {
            push_issue(
                issues,
                options,
                idx + 1,
                1,
                Severity::Error,
                "SYNTAX",
                "unterminated string or template literal on this line",
                line,
            );
        }
    }
}

fn has_unterminated_quote(line: &str, quote: char) -> bool {
    let mut count = 0usize;
    let mut esc = false;
    for ch in line.chars() {
        if esc {
            esc = false;
            continue;
        }
        if ch == '\\' {
            esc = true;
            continue;
        }
        if ch == quote {
            count += 1;
        }
    }
    count % 2 == 1
}

fn line_checks(code: &str, stripped: &str, options: &Options, issues: &mut Vec<Issue>) {
    let mut previous_was_terminal: Option<(usize, String)> = None;
    for (idx, (orig, clean)) in code.lines().zip(stripped.lines()).enumerate() {
        let line_no = idx + 1;
        let t = clean.trim();
        if t.is_empty() {
            continue;
        }
        if let Some((prev, _)) = &previous_was_terminal {
            if !t.starts_with('}') && options.enabled("UNREACHABLE") {
                push_issue(
                    issues,
                    options,
                    line_no,
                    first_col(orig),
                    Severity::Warning,
                    "UNREACHABLE",
                    format!("statement is unreachable after line {prev}"),
                    orig,
                );
            }
        }
        if contains_eqeq(clean) {
            push_issue(
                issues,
                options,
                line_no,
                clean.find("==").unwrap_or(0) + 1,
                Severity::Warning,
                "EQEQ",
                "use `===`/`!==` instead of loose equality",
                orig,
            );
        }
        if clean.contains("debugger") {
            push_issue(
                issues,
                options,
                line_no,
                clean.find("debugger").unwrap_or(0) + 1,
                Severity::Warning,
                "NO-DEBUGGER",
                "remove debugger statement",
                orig,
            );
        }
        if clean.contains("console.") {
            push_issue(
                issues,
                options,
                line_no,
                clean.find("console.").unwrap_or(0) + 1,
                Severity::Info,
                "NO-CONSOLE",
                "console call left in code",
                orig,
            );
        }
        if clean.contains("alert(") {
            push_issue(
                issues,
                options,
                line_no,
                clean.find("alert(").unwrap_or(0) + 1,
                Severity::Info,
                "NO-ALERT",
                "alert call left in code",
                orig,
            );
        }
        if options.ecma != "es5" && word_positions(clean, "var").next().is_some() {
            push_issue(
                issues,
                options,
                line_no,
                clean.find("var").unwrap_or(0) + 1,
                Severity::Info,
                "NO-VAR",
                "prefer `let` or `const` over `var` for modern JavaScript targets",
                orig,
            );
        }
        if needs_semicolon(t) {
            push_issue(
                issues,
                options,
                line_no,
                orig.len(),
                Severity::Info,
                "SEMICOLON",
                "statement appears to be missing a semicolon",
                orig,
            );
        }
        if needs_curly(t) {
            push_issue(
                issues,
                options,
                line_no,
                first_col(orig),
                Severity::Info,
                "CURLY",
                "wrap single-statement control flow in braces",
                orig,
            );
        }
        if options.source_type == "script" && (t.starts_with("import ") || t.starts_with("export "))
        {
            push_issue(
                issues,
                options,
                line_no,
                first_col(orig),
                Severity::Error,
                "MODULE-SYNTAX",
                "module syntax used while source_type=script",
                orig,
            );
        }
        if matches!(
            t,
            "return" | "return;" | "throw" | "break" | "break;" | "continue" | "continue;"
        ) || t.starts_with("return ")
            || t.starts_with("throw ")
            || t.starts_with("break ")
            || t.starts_with("continue ")
        {
            previous_was_terminal = Some((line_no, t.to_string()));
        } else if t.starts_with('}') {
            previous_was_terminal = None;
        }
    }
}

fn variable_checks(code: &str, stripped: &str, options: &Options, issues: &mut Vec<Issue>) {
    let mut declared: Vec<(String, usize, usize, String)> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for (idx, (orig, clean)) in code.lines().zip(stripped.lines()).enumerate() {
        let line_no = idx + 1;
        let tokens = identifiers(clean);
        let mut i = 0;
        while i < tokens.len() {
            let (tok, col) = &tokens[i];
            if matches!(tok.as_str(), "var" | "let" | "const" | "function") {
                if let Some((name, ncol)) = tokens.get(i + 1) {
                    declared.push((name.clone(), line_no, *ncol, orig.to_string()));
                }
                i += 2;
            } else {
                used.push(tok.clone());
                if looks_like_implicit_assignment(clean, tok) && !is_known(tok, &declared, options)
                {
                    push_issue(
                        issues,
                        options,
                        line_no,
                        *col,
                        Severity::Warning,
                        "UNDEF-VAR",
                        format!("`{tok}` is assigned without a declaration"),
                        orig,
                    );
                }
                i += 1;
            }
        }
    }
    for (name, line, col, src) in &declared {
        if !used.iter().any(|u| u == name) {
            push_issue(
                issues,
                options,
                *line,
                *col,
                Severity::Warning,
                "UNUSED-VAR",
                format!("`{name}` is declared but never used"),
                src,
            );
        }
    }
}

fn first_col(s: &str) -> usize {
    s.len() - s.trim_start().len() + 1
}

fn contains_eqeq(s: &str) -> bool {
    let b = s.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'='
            && b[i + 1] == b'='
            && b.get(i.wrapping_sub(1)) != Some(&b'=')
            && b.get(i + 2) != Some(&b'=')
        {
            return true;
        }
        if b[i] == b'!' && b[i + 1] == b'=' && b.get(i + 2) != Some(&b'=') {
            return true;
        }
    }
    false
}

fn needs_semicolon(t: &str) -> bool {
    if t.ends_with(';')
        || t.ends_with('{')
        || t.ends_with('}')
        || t.ends_with(',')
        || t.starts_with("//")
    {
        return false;
    }
    let starts = [
        "const ", "let ", "var ", "return ", "throw ", "break", "continue", "console.", "alert(",
    ];
    starts.iter().any(|p| t.starts_with(p)) || t.contains('=')
}

fn needs_curly(t: &str) -> bool {
    (t.starts_with("if ")
        || t.starts_with("if(")
        || t.starts_with("for ")
        || t.starts_with("for(")
        || t.starts_with("while ")
        || t.starts_with("while("))
        && !t.contains('{')
}

fn word_positions<'a>(line: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    line.match_indices(word).filter_map(move |(idx, _)| {
        let before = line[..idx].chars().last();
        let after = line[idx + word.len()..].chars().next();
        if before.map_or(true, |c| !is_ident(c)) && after.map_or(true, |c| !is_ident(c)) {
            Some(idx)
        } else {
            None
        }
    })
}

fn identifiers(line: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0usize;
    for (i, ch) in line.char_indices() {
        if cur.is_empty() {
            if ch == '_' || ch == '$' || ch.is_ascii_alphabetic() {
                cur.push(ch);
                start = i + 1;
            }
        } else if is_ident(ch) {
            cur.push(ch);
        } else {
            out.push((cur.clone(), start));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push((cur, start));
    }
    out.into_iter().filter(|(s, _)| !is_keyword(s)).collect()
}

fn is_ident(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphanumeric()
}
fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "return"
            | "throw"
            | "try"
            | "catch"
            | "finally"
            | "new"
            | "class"
            | "extends"
            | "import"
            | "export"
            | "from"
            | "default"
            | "async"
            | "await"
            | "true"
            | "false"
            | "null"
            | "undefined"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
    )
}

fn looks_like_implicit_assignment(line: &str, name: &str) -> bool {
    if let Some(pos) = line.find(name) {
        let rest = line[pos + name.len()..].trim_start();
        rest.starts_with('=') && !rest.starts_with("==") && !line[..pos].contains('.')
    } else {
        false
    }
}

fn is_known(name: &str, declared: &[(String, usize, usize, String)], options: &Options) -> bool {
    if declared.iter().any(|(n, _, _, _)| n == name) {
        return true;
    }
    let browser = [
        "window",
        "document",
        "console",
        "alert",
        "fetch",
        "setTimeout",
        "clearTimeout",
        "URL",
    ];
    let node = [
        "require",
        "module",
        "exports",
        "process",
        "Buffer",
        "__dirname",
        "__filename",
        "console",
    ];
    match options.env.as_str() {
        "browser" => browser.contains(&name),
        "node" => node.contains(&name),
        "both" => browser.contains(&name) || node.contains(&name),
        _ => false,
    }
}

fn format_text(issues: &[Issue]) -> String {
    if issues.is_empty() {
        return "No JavaScript lint issues found.".into();
    }
    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let infos = issues
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();
    let mut out = format!(
        "{} issue(s): {errors} error(s), {warnings} warning(s), {infos} info(s)\n",
        issues.len()
    );
    for i in issues {
        out.push_str(&format!(
            "{}:{}  {}  {}  {}\n",
            i.line,
            i.col,
            i.severity.as_str(),
            i.rule,
            i.message
        ));
        if !i.source.is_empty() {
            out.push_str(&format!("    {}\n", i.source));
        }
    }
    out.trim_end().to_string()
}

fn format_json(issues: &[Issue]) -> String {
    let mut out = String::from("{\"issues\":[");
    for (idx, i) in issues.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!("{{\"line\":{},\"column\":{},\"severity\":\"{}\",\"rule\":\"{}\",\"message\":\"{}\",\"source\":\"{}\"}}", i.line, i.col, i.severity.as_str(), i.rule, esc(&i.message), esc(&i.source)));
    }
    out.push_str("]}");
    out
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_common_issues() {
        let code = "var unused = 1\nif (x == 1) console.log(x)\nreturn 1;\nalert('later');";
        let out = run_with_options(
            code,
            "recommended",
            "latest",
            "browser",
            "auto",
            "all",
            "",
            "text",
        )
        .unwrap();
        assert!(out.contains("NO-VAR"));
        assert!(out.contains("UNUSED-VAR"));
        assert!(out.contains("EQEQ"));
        assert!(out.contains("CURLY"));
        assert!(out.contains("UNREACHABLE"));
    }

    #[test]
    fn json_and_ignore_work() {
        let out = run_with_options(
            "const a = 1",
            "recommended",
            "latest",
            "browser",
            "auto",
            "all",
            "UNUSED-VAR SEMICOLON",
            "json",
        )
        .unwrap();
        assert_eq!(out, "{\"issues\":[]}");
    }

    #[test]
    fn validates_options_and_empty_input() {
        assert!(run_with_options(
            "",
            "recommended",
            "latest",
            "browser",
            "auto",
            "all",
            "",
            "text"
        )
        .is_err());
        assert!(
            run_with_options("x", "bad", "latest", "browser", "auto", "all", "", "text").is_err()
        );
    }
}
