//! sql-linter core — pure compute shared by the chat skill block and web page.
//!
//! Heuristic, offline SQL linting for pasted statements. It does not execute SQL
//! and intentionally stays smaller than a full dialect parser: structural syntax
//! checks plus common query anti-patterns that can be detected reliably with
//! token masking and line-aware scans.

use regex::Regex;
use serde_json::json;
use std::collections::BTreeSet;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    fn parse_min(s: &str) -> Result<Severity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" | "info" => Severity::Info,
            "warning" | "warn" => Severity::Warning,
            "error" | "errors" => Severity::Error,
            other => {
                return Err(format!(
                    "unknown min_severity '{other}' (use all, warning, or error)"
                ))
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "report" => Format::Text,
            "json" => Format::Json,
            other => return Err(format!("unknown format '{other}' (use text or json)")),
        })
    }
}

#[derive(Debug, Clone)]
struct Finding {
    line: usize,
    code: &'static str,
    severity: Severity,
    message: String,
    snippet: String,
}

/// Lint SQL text.
///
/// - `dialect`: generic | mysql | postgresql | sqlite | tsql. Currently affects
///   comment masking (MySQL `#` line comments) and is reported for clarity.
/// - `min_severity`: all/info | warning | error.
/// - `ignore`: comma/space separated rule codes to suppress.
/// - `format`: text | json.
pub fn lint(
    sql: &str,
    dialect: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, String> {
    if sql.trim().is_empty() {
        return Err("SQL input is empty".into());
    }
    let dialect = parse_dialect(dialect)?;
    let min = Severity::parse_min(min_severity)?;
    let format = Format::parse(format)?;
    let ignored = parse_ignore(ignore);

    let masked = mask_sql(sql, dialect == "mysql");
    let mut findings = Vec::new();
    structural_checks(sql, &masked, &mut findings);
    anti_pattern_checks(sql, &masked, &mut findings);

    findings.retain(|f| f.severity >= min && !ignored.contains(f.code));
    findings.sort_by(|a, b| a.line.cmp(&b.line).then(a.code.cmp(b.code)));

    match format {
        Format::Text => Ok(render_text(dialect, &findings)),
        Format::Json => Ok(render_json(dialect, &findings)),
    }
}

fn parse_dialect(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "generic" | "ansi" => "generic",
        "mysql" | "mariadb" => "mysql",
        "postgresql" | "postgres" | "pg" => "postgresql",
        "sqlite" => "sqlite",
        "tsql" | "sqlserver" | "mssql" => "tsql",
        other => {
            return Err(format!(
                "unknown dialect '{other}' (use generic, mysql, postgresql, sqlite, or tsql)"
            ))
        }
    })
}

fn parse_ignore(s: &str) -> BTreeSet<&'static str> {
    let mut set = BTreeSet::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()) {
        match part.trim().to_ascii_uppercase().as_str() {
            "SELECT-STAR" => {
                set.insert("SELECT-STAR");
            }
            "IMPLICIT-JOIN" => {
                set.insert("IMPLICIT-JOIN");
            }
            "SUBQUERY-NO-ALIAS" => {
                set.insert("SUBQUERY-NO-ALIAS");
            }
            "BARE-JOIN" => {
                set.insert("BARE-JOIN");
            }
            "SYNTAX" => {
                set.insert("SYNTAX");
            }
            _ => {}
        }
    }
    set
}

/// Replace comments and string literals with spaces, preserving newlines and byte
/// length so regex match positions can map back to the original line/snippet.
fn mask_sql(sql: &str, mysql_hash_comment: bool) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' || b == b'"' || b == b'`' {
            let quote = b;
            out.push(' ');
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                out.push(if c == b'\n' { '\n' } else { ' ' });
                i += 1;
                if c == quote {
                    if quote == b'\'' && i < bytes.len() && bytes[i] == b'\'' {
                        out.push(' ');
                        i += 1;
                        continue;
                    }
                    break;
                }
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
        } else if mysql_hash_comment && b == b'#' {
            out.push(' ');
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < bytes.len() {
                let c = bytes[i];
                out.push(if c == b'\n' { '\n' } else { ' ' });
                i += 1;
                if c == b'*' && i < bytes.len() && bytes[i] == b'/' {
                    out.push(' ');
                    i += 1;
                    break;
                }
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

fn structural_checks(original: &str, masked: &str, findings: &mut Vec<Finding>) {
    let mut depth = 0i32;
    for (idx, ch) in masked.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    push(
                        findings,
                        original,
                        idx,
                        "SYNTAX",
                        Severity::Error,
                        "closing parenthesis without a matching opening parenthesis",
                    );
                } else {
                    depth -= 1;
                }
            }
            _ => {}
        }
    }
    if depth > 0 {
        push(
            findings,
            original,
            original.len().saturating_sub(1),
            "SYNTAX",
            Severity::Error,
            "unclosed parenthesis",
        );
    }

    if unterminated_single_quote(original) {
        push(
            findings,
            original,
            original.len().saturating_sub(1),
            "SYNTAX",
            Severity::Error,
            "unterminated string literal",
        );
    }
    if unterminated_block_comment(original) {
        push(
            findings,
            original,
            original.len().saturating_sub(1),
            "SYNTAX",
            Severity::Error,
            "unterminated block comment",
        );
    }

    let leading_comma = Regex::new(r"(?i)(select|by)\s*,").unwrap();
    for m in leading_comma.find_iter(masked) {
        push(
            findings,
            original,
            m.start(),
            "SYNTAX",
            Severity::Error,
            "leading comma in a SELECT/GROUP/ORDER list",
        );
    }
    let trailing_comma =
        Regex::new(r"(?i),\s*(from|where|group\s+by|order\s+by|having|limit|;|$)").unwrap();
    for m in trailing_comma.find_iter(masked) {
        push(
            findings,
            original,
            m.start(),
            "SYNTAX",
            Severity::Error,
            "trailing comma before the next SQL clause",
        );
    }
}

fn anti_pattern_checks(original: &str, masked: &str, findings: &mut Vec<Finding>) {
    let select_star =
        Regex::new(r"(?i)\bselect\b(?s:.*?)((?:\b[a-z_][a-z0-9_]*\s*\.\s*)?\*)").unwrap();
    for m in select_star.find_iter(masked) {
        if let Some(rel) = m.as_str().find('*') {
            push(
                findings,
                original,
                m.start() + rel,
                "SELECT-STAR",
                Severity::Warning,
                "avoid SELECT *; list the columns needed so schemas and payloads stay stable",
            );
        }
    }

    let implicit_join =
        Regex::new(r"(?i)\bfrom\s+[a-z_][\w.]*\s+(?:as\s+)?[a-z_][\w]*\s*,\s*[a-z_][\w.]*")
            .unwrap();
    for m in implicit_join.find_iter(masked) {
        push(
            findings,
            original,
            m.start(),
            "IMPLICIT-JOIN",
            Severity::Warning,
            "comma-separated tables are an implicit join; use explicit JOIN ... ON clauses",
        );
    }

    let bare_join = Regex::new(r"(?i)(^|\s)join\s+[a-z_(]").unwrap();
    for m in bare_join.find_iter(masked) {
        let join_at = m.start() + m.as_str().to_ascii_lowercase().find("join").unwrap_or(0);
        push(
            findings,
            original,
            join_at,
            "BARE-JOIN",
            Severity::Info,
            "bare JOIN leaves the join type implicit; write INNER JOIN, LEFT JOIN, etc.",
        );
    }

    find_subqueries_without_alias(original, masked, findings);
}

fn find_subqueries_without_alias(original: &str, masked: &str, findings: &mut Vec<Finding>) {
    let lower = masked.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(pos) = lower[i..].find("from") {
        let start = i + pos;
        let after = start + 4;
        if !is_word_boundary(bytes, start, after) {
            i = after;
            continue;
        }
        let mut j = after;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'(' {
            i = after;
            continue;
        }
        let Some(close) = matching_paren(masked, j) else {
            i = after;
            continue;
        };
        let inside = &lower[j + 1..close];
        if !inside.contains("select") {
            i = close + 1;
            continue;
        }
        let mut k = close + 1;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        if lower[k..].starts_with("as ") {
            i = close + 1;
            continue;
        }
        let next = lower[k..].split_whitespace().next().unwrap_or("");
        let clause = matches!(
            next,
            "where"
                | "join"
                | "inner"
                | "left"
                | "right"
                | "full"
                | "cross"
                | "group"
                | "order"
                | "having"
                | "limit"
                | "union"
                | ";"
        );
        if next.is_empty() || clause {
            push(
                findings,
                original,
                j,
                "SUBQUERY-NO-ALIAS",
                Severity::Warning,
                "derived table/subquery in FROM should have an alias",
            );
        }
        i = close + 1;
    }
}

fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let before =
        start == 0 || !bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b'_';
    let after = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_';
    before && after
}

fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in s[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn unterminated_single_quote(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut in_quote = false;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if in_quote && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            in_quote = !in_quote;
        }
        i += 1;
    }
    in_quote
}

fn unterminated_block_comment(sql: &str) -> bool {
    let mut depth = 0i32;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' && depth > 0 {
            depth -= 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    depth > 0
}

fn push(
    findings: &mut Vec<Finding>,
    original: &str,
    byte: usize,
    code: &'static str,
    severity: Severity,
    message: &str,
) {
    let line = original[..byte.min(original.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1;
    let snippet = original
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim()
        .to_string();
    findings.push(Finding {
        line,
        code,
        severity,
        message: message.to_string(),
        snippet,
    });
}

fn render_text(dialect: &str, findings: &[Finding]) -> String {
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();
    let mut out = format!(
        "SQL lint ({dialect}) · {} findings · {errors} errors · {warnings} warnings · {info} info",
        findings.len()
    );
    if findings.is_empty() {
        out.push_str("\n\nNo issues found.");
        return out;
    }
    for f in findings {
        let _ = write!(
            out,
            "\n\nL{} [{}] {}: {}",
            f.line,
            f.severity.label(),
            f.code,
            f.message
        );
        if !f.snippet.is_empty() {
            let _ = write!(out, "\n  {}", f.snippet);
        }
    }
    out
}

fn render_json(dialect: &str, findings: &[Finding]) -> String {
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "line": f.line,
                "code": f.code,
                "severity": f.severity.label(),
                "message": f.message,
                "snippet": f.snippet,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "dialect": dialect,
        "summary": { "findings": findings.len(), "errors": errors, "warnings": warnings, "info": info },
        "findings": items,
    })).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAD: &str = "SELECT *\nFROM users u, orders o\nJOIN payments p ON p.order_id = o.id\nWHERE u.id = o.user_id";

    #[test]
    fn flags_select_star_implicit_join_and_bare_join() {
        let out = lint(BAD, "generic", "all", "", "text").unwrap();
        assert!(
            out.starts_with("SQL lint (generic) · 3 findings · 0 errors · 2 warnings · 1 info"),
            "{out}"
        );
        assert!(out.contains("SELECT-STAR"), "{out}");
        assert!(out.contains("IMPLICIT-JOIN"), "{out}");
        assert!(out.contains("BARE-JOIN"), "{out}");
    }

    #[test]
    fn min_severity_and_ignore_filter_findings() {
        let out = lint(BAD, "generic", "warning", "SELECT-STAR", "text").unwrap();
        assert!(out.starts_with("SQL lint (generic) · 1 findings"), "{out}");
        assert!(!out.contains("SELECT-STAR"), "{out}");
        assert!(out.contains("IMPLICIT-JOIN"), "{out}");
        assert!(!out.contains("BARE-JOIN"), "{out}");
    }

    #[test]
    fn structural_syntax_errors_are_errors() {
        let out = lint(
            "SELECT id, FROM users WHERE name = 'Ada",
            "generic",
            "all",
            "",
            "text",
        )
        .unwrap();
        assert!(out.contains("SYNTAX"), "{out}");
        assert!(out.contains("trailing comma"), "{out}");
        assert!(out.contains("unterminated string"), "{out}");
    }

    #[test]
    fn subquery_without_alias_is_flagged() {
        let out = lint(
            "SELECT x FROM (SELECT id FROM users) WHERE id > 1",
            "generic",
            "all",
            "",
            "text",
        )
        .unwrap();
        assert!(out.contains("SUBQUERY-NO-ALIAS"), "{out}");
    }

    #[test]
    fn json_output_is_valid() {
        let out = lint(BAD, "postgresql", "all", "", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["dialect"], "postgresql");
        assert_eq!(v["summary"]["findings"], 3);
    }

    #[test]
    fn comments_and_strings_are_masked() {
        let sql = "SELECT 'not *' AS s -- SELECT *\nFROM users";
        let out = lint(sql, "generic", "all", "", "text").unwrap();
        assert!(out.contains("No issues found"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(lint("  \n", "generic", "all", "", "text")
            .unwrap_err()
            .contains("empty"));
    }

    #[test]
    fn bad_dialect_errors() {
        assert!(lint("select 1", "oracle", "all", "", "text")
            .unwrap_err()
            .contains("unknown dialect"));
    }
}
