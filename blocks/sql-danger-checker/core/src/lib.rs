//! sql-danger-checker core — pure compute, shared by the chat skill block and the web page.
//!
//! A conservative, static scanner for *destructive* SQL. It never connects to a
//! database and never runs anything — it reads raw SQL text and flags the
//! statements that can lose or overwrite data before you execute them:
//!
//!   * `DROP DATABASE` / `DROP SCHEMA` / `DROP TABLE`, and `TRUNCATE` — critical;
//!   * `DROP` of other objects (index/view/…) and `ALTER … DROP …` — high;
//!   * other `ALTER` (schema change) — medium;
//!   * `DELETE` / `UPDATE` with no `WHERE` clause (or a trivially-true one such as
//!     `WHERE 1=1`) — high, because they touch every row.
//!
//! To avoid obvious false positives it first *masks* line comments (`--`, plus
//! `#` in MySQL), block comments (`/* … */`), and the contents of string /
//! identifier literals (`'…'`, `"…"`, `` `…` ``) with spaces — so a `DROP` inside
//! a comment or string is ignored, and a `;` inside a string does not split a
//! statement. It is a heuristic, not a parser: treat findings as leads to review.
//! No wafer/wasm-bindgen deps.

use regex::Regex;
use serde_json::json;

/// Severity of a finding, ordered least → most dangerous.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Severity::Low => "LOW",
            Severity::Medium => "MEDIUM",
            Severity::High => "HIGH",
            Severity::Critical => "CRITICAL",
        }
    }
}

/// Minimum severity to include in the report.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MinSeverity {
    All,
    Medium,
    High,
    Critical,
}

impl MinSeverity {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" | "low" => Ok(MinSeverity::All),
            "medium" => Ok(MinSeverity::Medium),
            "high" => Ok(MinSeverity::High),
            "critical" => Ok(MinSeverity::Critical),
            other => Err(format!(
                "invalid min_severity {other:?}: expected one of all, medium, high, critical"
            )),
        }
    }
    fn keeps(self, s: Severity) -> bool {
        let floor = match self {
            MinSeverity::All => Severity::Low,
            MinSeverity::Medium => Severity::Medium,
            MinSeverity::High => Severity::High,
            MinSeverity::Critical => Severity::Critical,
        };
        s >= floor
    }
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            other => Err(format!("invalid format {other:?}: expected \"text\" or \"json\"")),
        }
    }
}

/// SQL dialect hint. Only affects lexing today: MySQL additionally treats `#` as
/// a line comment. Every dialect shares the same destructive-statement ruleset.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Generic,
    Mysql,
    Postgresql,
    Sqlite,
    Tsql,
}

impl Dialect {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "generic" | "standard" | "ansi" => Ok(Dialect::Generic),
            "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "postgresql" | "postgres" | "pg" => Ok(Dialect::Postgresql),
            "sqlite" => Ok(Dialect::Sqlite),
            "tsql" | "sqlserver" | "mssql" => Ok(Dialect::Tsql),
            other => Err(format!(
                "invalid dialect {other:?}: expected one of generic, mysql, postgresql, sqlite, tsql"
            )),
        }
    }
    fn hash_is_comment(self) -> bool {
        self == Dialect::Mysql
    }
}

/// Which destructive categories the caller has explicitly allowed (suppressed).
struct Allow {
    drop: bool,
    truncate: bool,
    alter: bool,
    delete: bool,
    update: bool,
}

impl Allow {
    fn parse(s: &str) -> Result<Self, String> {
        let mut a = Allow {
            drop: false,
            truncate: false,
            alter: false,
            delete: false,
            update: false,
        };
        for tok in s.split(|c: char| c == ',' || c.is_whitespace()) {
            let t = tok.trim().to_ascii_lowercase();
            if t.is_empty() {
                continue;
            }
            match t.as_str() {
                "drop" => a.drop = true,
                "truncate" => a.truncate = true,
                "alter" => a.alter = true,
                "delete" => a.delete = true,
                "update" => a.update = true,
                other => {
                    return Err(format!(
                        "invalid allow entry {other:?}: expected a comma-separated list of \
                         drop, truncate, alter, delete, update"
                    ))
                }
            }
        }
        Ok(a)
    }
    fn allows(&self, cat: &str) -> bool {
        match cat {
            "drop" => self.drop,
            "truncate" => self.truncate,
            "alter" => self.alter,
            "delete" => self.delete,
            "update" => self.update,
            _ => false,
        }
    }
}

/// One flagged statement.
pub struct Finding {
    pub statement: usize,
    pub line: usize,
    pub severity: Severity,
    pub rule: &'static str,
    pub category: &'static str,
    pub message: &'static str,
    pub snippet: String,
}

/// Entry point: scan `sql` for destructive statements and render the report in
/// `format`, filtered to `min_severity`, lexed for `dialect`, suppressing the
/// categories in `allow`. With `strict` = "true"/"1"/"yes", even *guarded*
/// `DELETE`/`UPDATE` (they have a real `WHERE`) are reported as low so every
/// destructive statement gets an explicit look before you run it.
pub fn run(
    sql: &str,
    dialect: &str,
    min_severity: &str,
    strict: &str,
    allow: &str,
    format: &str,
) -> Result<String, String> {
    if sql.trim().is_empty() {
        return Err("no SQL provided".into());
    }
    let dia = Dialect::parse(dialect)?;
    let min = MinSeverity::parse(min_severity)?;
    let allow = Allow::parse(allow)?;
    let fmt = Format::parse(format)?;
    let strict = parse_bool(strict)?;

    let (statements_scanned, findings) = scan(sql, dia, strict);
    let findings: Vec<Finding> = findings
        .into_iter()
        .filter(|f| min.keeps(f.severity))
        .filter(|f| !allow.allows(f.category))
        .collect();

    Ok(match fmt {
        Format::Text => render_text(&findings, statements_scanned),
        Format::Json => render_json(&findings, statements_scanned),
    })
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "false" | "0" | "no" | "off" => Ok(false),
        "true" | "1" | "yes" | "on" => Ok(true),
        other => Err(format!("invalid strict {other:?}: expected true or false")),
    }
}

/// Replace comment bodies and string/identifier literal contents with spaces so
/// the classifier only ever sees real SQL keywords and real statement
/// terminators. Newlines are preserved 1:1 so line numbers stay accurate.
fn mask(chars: &[char], dia: Dialect) -> Vec<char> {
    #[derive(PartialEq)]
    enum St {
        Normal,
        Sq,
        Dq,
        Bt,
        Line,
        Block,
    }
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut st = St::Normal;
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < chars.len() {
        let c = chars[i];
        let n = chars.get(i + 1).copied();
        match st {
            St::Normal => {
                if c == '-' && n == Some('-') {
                    st = St::Line;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if c == '/' && n == Some('*') {
                    st = St::Block;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if c == '#' && dia.hash_is_comment() {
                    st = St::Line;
                    out.push(' ');
                    i += 1;
                } else {
                    match c {
                        '\'' => st = St::Sq,
                        '"' => st = St::Dq,
                        '`' => st = St::Bt,
                        _ => {}
                    }
                    out.push(c);
                    i += 1;
                }
            }
            St::Line => {
                if c == '\n' {
                    st = St::Normal;
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            St::Block => {
                if c == '*' && n == Some('/') {
                    st = St::Normal;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else {
                    out.push(blank(c));
                    i += 1;
                }
            }
            St::Sq => {
                out.push(if c == '\'' { '\'' } else { blank(c) });
                if c == '\'' {
                    st = St::Normal;
                }
                i += 1;
            }
            St::Dq => {
                out.push(if c == '"' { '"' } else { blank(c) });
                if c == '"' {
                    st = St::Normal;
                }
                i += 1;
            }
            St::Bt => {
                out.push(if c == '`' { '`' } else { blank(c) });
                if c == '`' {
                    st = St::Normal;
                }
                i += 1;
            }
        }
    }
    out
}

fn scan(sql: &str, dia: Dialect, strict: bool) -> (usize, Vec<Finding>) {
    let chars: Vec<char> = sql.chars().collect();
    let masked = mask(&chars, dia);

    // Statement boundaries: unmasked ';' only (in-string/in-comment ';' is now a space).
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for i in 0..masked.len() {
        if masked[i] == ';' {
            ranges.push((start, i));
            start = i + 1;
        }
    }
    if start < masked.len() {
        ranges.push((start, masked.len()));
    }

    let where_re = Regex::new(r"(?i)\bwhere\b").unwrap();
    // A trivially-true WHERE that still touches every row.
    let tautology_re =
        Regex::new(r"(?i)\bwhere\s+(?:1\s*=\s*1|'1'\s*=\s*'1'|true)\s*$").unwrap();
    let drop_db_re = Regex::new(r"(?i)\bdrop\s+(database|schema)\b").unwrap();
    let drop_table_re = Regex::new(r"(?i)\bdrop\s+table\b").unwrap();
    let alter_drop_re = Regex::new(r"(?i)\balter\b.*\bdrop\b").unwrap();

    let mut statements_scanned = 0;
    let mut findings = Vec::new();
    let mut stmt_no = 0;

    for (s, e) in ranges {
        let masked_stmt: String = masked[s..e].iter().collect();
        if masked_stmt.trim().is_empty() {
            continue;
        }
        statements_scanned += 1;
        stmt_no += 1;

        // Line of the statement's first non-whitespace character.
        let first = (s..e).find(|&k| !chars[k].is_whitespace());
        let line = match first {
            Some(idx) => chars[..idx].iter().filter(|&&c| c == '\n').count() + 1,
            None => 1,
        };
        let orig_stmt: String = chars[s..e].iter().collect();

        if let Some((severity, rule, category, message)) = classify(
            &masked_stmt,
            &where_re,
            &tautology_re,
            &drop_db_re,
            &drop_table_re,
            &alter_drop_re,
            strict,
        ) {
            findings.push(Finding {
                statement: stmt_no,
                line,
                severity,
                rule,
                category,
                message,
                snippet: snippet_of(&orig_stmt),
            });
        }
    }

    (statements_scanned, findings)
}

/// The leading SQL keyword of a (masked) statement, uppercased.
fn first_keyword(m: &str) -> Option<String> {
    m.split(|c: char| !c.is_ascii_alphabetic())
        .find(|w| !w.is_empty())
        .map(|w| w.to_ascii_uppercase())
}

/// Classify a single masked statement into at most one finding.
#[allow(clippy::too_many_arguments)]
fn classify(
    m: &str,
    where_re: &Regex,
    tautology_re: &Regex,
    drop_db_re: &Regex,
    drop_table_re: &Regex,
    alter_drop_re: &Regex,
    strict: bool,
) -> Option<(Severity, &'static str, &'static str, &'static str)> {
    let lower = m.to_ascii_lowercase();
    let trimmed_lower = lower.trim();
    let kw = first_keyword(m)?;

    match kw.as_str() {
        "DROP" => {
            if drop_db_re.is_match(&lower) {
                Some((
                    Severity::Critical,
                    "DROP-DATABASE",
                    "drop",
                    "DROP DATABASE/SCHEMA permanently deletes the entire database and every object in it.",
                ))
            } else if drop_table_re.is_match(&lower) {
                Some((
                    Severity::Critical,
                    "DROP-TABLE",
                    "drop",
                    "DROP TABLE permanently removes the table and all of its data.",
                ))
            } else {
                Some((
                    Severity::High,
                    "DROP-OBJECT",
                    "drop",
                    "DROP removes a database object (index, view, column, …) and any data it holds.",
                ))
            }
        }
        "TRUNCATE" => Some((
            Severity::Critical,
            "TRUNCATE",
            "truncate",
            "TRUNCATE deletes every row in the table and usually cannot be rolled back.",
        )),
        "ALTER" => {
            if alter_drop_re.is_match(&lower) {
                Some((
                    Severity::High,
                    "ALTER-DROP",
                    "alter",
                    "ALTER … DROP removes a column or constraint and discards the data stored in it.",
                ))
            } else {
                Some((
                    Severity::Medium,
                    "ALTER",
                    "alter",
                    "ALTER changes the schema; review it (and back up) before running against production.",
                ))
            }
        }
        "DELETE" => {
            if !where_re.is_match(&lower) {
                Some((
                    Severity::High,
                    "DELETE-NO-WHERE",
                    "delete",
                    "DELETE without a WHERE clause removes ALL rows in the table.",
                ))
            } else if tautology_re.is_match(trimmed_lower) {
                Some((
                    Severity::High,
                    "DELETE-TRUE-WHERE",
                    "delete",
                    "DELETE with an always-true WHERE (e.g. WHERE 1=1) still removes ALL rows.",
                ))
            } else if strict {
                Some((
                    Severity::Low,
                    "DELETE-CONFIRM",
                    "delete",
                    "Destructive DELETE — confirm the WHERE clause targets only the rows you intend.",
                ))
            } else {
                None
            }
        }
        "UPDATE" => {
            if !where_re.is_match(&lower) {
                Some((
                    Severity::High,
                    "UPDATE-NO-WHERE",
                    "update",
                    "UPDATE without a WHERE clause overwrites ALL rows in the table.",
                ))
            } else if tautology_re.is_match(trimmed_lower) {
                Some((
                    Severity::High,
                    "UPDATE-TRUE-WHERE",
                    "update",
                    "UPDATE with an always-true WHERE (e.g. WHERE 1=1) still overwrites ALL rows.",
                ))
            } else if strict {
                Some((
                    Severity::Low,
                    "UPDATE-CONFIRM",
                    "update",
                    "Destructive UPDATE — confirm the WHERE clause targets only the rows you intend.",
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn snippet_of(stmt: &str) -> String {
    // Collapse internal whitespace/newlines to a single-line, readable snippet.
    let one_line: String = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 200 {
        let cut: String = one_line.chars().take(200).collect();
        format!("{cut}…")
    } else {
        one_line
    }
}

fn counts(findings: &[Finding]) -> (usize, usize, usize, usize) {
    let mut c = 0;
    let mut h = 0;
    let mut m = 0;
    let mut l = 0;
    for f in findings {
        match f.severity {
            Severity::Critical => c += 1,
            Severity::High => h += 1,
            Severity::Medium => m += 1,
            Severity::Low => l += 1,
        }
    }
    (c, h, m, l)
}

fn render_text(findings: &[Finding], statements_scanned: usize) -> String {
    if findings.is_empty() {
        return format!(
            "No dangerous statements found in {statements_scanned} statement(s) scanned. \
             (Static heuristic — not a guarantee; still review before running.)"
        );
    }
    let (c, h, m, l) = counts(findings);
    let mut out = String::new();
    out.push_str(&format!(
        "{} danger(s) found ({c} critical, {h} high, {m} medium, {l} low) in {statements_scanned} statement(s) scanned\n\n",
        findings.len()
    ));
    for f in findings {
        out.push_str(&format!(
            "statement {}, line {}  {}  {}  {}\n  {}\n\n",
            f.statement,
            f.line,
            f.severity.tag(),
            f.rule,
            f.message,
            f.snippet
        ));
    }
    out.push_str(
        "Before running: wrap these in a transaction (BEGIN … COMMIT/ROLLBACK), take a backup, \
         and add a WHERE clause or LIMIT where you meant to target specific rows.",
    );
    out
}

fn render_json(findings: &[Finding], statements_scanned: usize) -> String {
    let (c, h, m, l) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "statement": f.statement,
                "line": f.line,
                "severity": f.severity.label(),
                "rule": f.rule,
                "category": f.category,
                "message": f.message,
                "snippet": f.snippet,
            })
        })
        .collect();
    let v = json!({
        "summary": {
            "findings": findings.len(),
            "critical": c,
            "high": h,
            "medium": m,
            "low": l,
            "statements_scanned": statements_scanned,
        },
        "findings": items,
    });
    serde_json::to_string_pretty(&v).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(sql: &str) -> String {
        run(sql, "generic", "all", "false", "", "text").unwrap()
    }

    #[test]
    fn flags_drop_table_critical() {
        let out = text("DROP TABLE users;");
        assert!(out.contains("DROP-TABLE"), "{out}");
        assert!(out.contains("CRITICAL"), "{out}");
        assert!(
            out.contains("1 danger(s) found (1 critical, 0 high, 0 medium, 0 low)"),
            "{out}"
        );
        assert!(out.contains("statement 1, line 1"), "{out}");
    }

    #[test]
    fn flags_delete_without_where() {
        let out = text("DELETE FROM sessions;");
        assert!(out.contains("DELETE-NO-WHERE"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn flags_update_without_where() {
        let out = text("UPDATE accounts SET balance = 0;");
        assert!(out.contains("UPDATE-NO-WHERE"), "{out}");
    }

    #[test]
    fn delete_with_where_is_clean() {
        let out = text("DELETE FROM sessions WHERE id = 5;");
        assert!(out.contains("No dangerous statements found"), "{out}");
    }

    #[test]
    fn flags_truncate_critical() {
        let out = text("TRUNCATE TABLE audit_log;");
        assert!(out.contains("TRUNCATE"), "{out}");
        assert!(out.contains("CRITICAL"), "{out}");
    }

    #[test]
    fn flags_drop_database_critical() {
        let out = text("DROP DATABASE prod;");
        assert!(out.contains("DROP-DATABASE"), "{out}");
        assert!(out.contains("CRITICAL"), "{out}");
    }

    #[test]
    fn drop_index_is_high_object() {
        let out = text("DROP INDEX idx_users_email;");
        assert!(out.contains("DROP-OBJECT"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn alter_drop_column_is_high() {
        let out = text("ALTER TABLE users DROP COLUMN email;");
        assert!(out.contains("ALTER-DROP"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn alter_add_column_is_medium() {
        let out = text("ALTER TABLE users ADD COLUMN nickname text;");
        assert!(out.contains("ALTER"), "{out}");
        assert!(out.contains("MEDIUM"), "{out}");
    }

    #[test]
    fn always_true_where_still_flagged() {
        let out = text("DELETE FROM users WHERE 1=1;");
        assert!(out.contains("DELETE-TRUE-WHERE"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn plain_select_is_clean() {
        let out = text("SELECT * FROM users WHERE id = 1;");
        assert!(out.contains("No dangerous statements found"), "{out}");
        assert!(out.contains("1 statement(s) scanned"), "{out}");
    }

    #[test]
    fn drop_inside_comment_ignored() {
        let out = text("SELECT 1; -- DROP TABLE users");
        assert!(out.contains("No dangerous statements found"), "{out}");
    }

    #[test]
    fn drop_inside_block_comment_ignored() {
        let out = text("SELECT 1 /* TRUNCATE audit */ FROM t;");
        assert!(out.contains("No dangerous statements found"), "{out}");
    }

    #[test]
    fn keyword_inside_string_ignored() {
        let out = text("INSERT INTO logs (msg) VALUES ('DROP TABLE users');");
        assert!(out.contains("No dangerous statements found"), "{out}");
    }

    #[test]
    fn semicolon_inside_string_not_a_split() {
        // The ';' is inside a string, so this is ONE guarded UPDATE → clean.
        let out = text("UPDATE t SET note = 'a;b' WHERE id = 1;");
        assert!(out.contains("No dangerous statements found"), "{out}");
    }

    #[test]
    fn multiple_statements_counted_and_numbered() {
        let out = text("DELETE FROM a; SELECT 1; DROP TABLE b;");
        assert!(out.contains("3 statement(s) scanned"), "{out}");
        assert!(out.contains("2 danger(s) found"), "{out}");
        assert!(out.contains("DELETE-NO-WHERE"), "{out}");
        assert!(out.contains("DROP-TABLE"), "{out}");
    }

    #[test]
    fn min_severity_critical_hides_high() {
        let out = run("DELETE FROM a;", "generic", "critical", "false", "", "text").unwrap();
        assert!(out.contains("No dangerous statements found"), "{out}");
        let out2 = run("DROP TABLE a;", "generic", "critical", "false", "", "text").unwrap();
        assert!(out2.contains("DROP-TABLE"), "{out2}");
    }

    #[test]
    fn strict_flags_guarded_delete() {
        let out = run("DELETE FROM a WHERE id = 1;", "generic", "all", "true", "", "text").unwrap();
        assert!(out.contains("DELETE-CONFIRM"), "{out}");
        assert!(out.contains("LOW"), "{out}");
    }

    #[test]
    fn allow_suppresses_category() {
        let out = run("TRUNCATE t;", "generic", "all", "false", "truncate", "text").unwrap();
        assert!(out.contains("No dangerous statements found"), "{out}");
        let out2 = run(
            "TRUNCATE t; DROP TABLE u;",
            "generic",
            "all",
            "false",
            "truncate",
            "text",
        )
        .unwrap();
        assert!(out2.contains("DROP-TABLE"), "{out2}");
        assert!(!out2.contains("TRUNCATE  "), "{out2}");
    }

    #[test]
    fn mysql_hash_comment_masked() {
        let generic = text("SELECT 1 # DROP TABLE x");
        // In generic '#' is not a comment, but the statement still starts with SELECT → clean.
        assert!(generic.contains("No dangerous statements found"), "{generic}");
        let mysql = run(
            "DROP TABLE keep; # DELETE FROM x",
            "mysql",
            "all",
            "false",
            "",
            "text",
        )
        .unwrap();
        assert!(mysql.contains("DROP-TABLE"), "{mysql}");
        assert!(!mysql.contains("DELETE"), "{mysql}");
    }

    #[test]
    fn json_output_shape() {
        let out = run(
            "DROP TABLE users; DELETE FROM logs;",
            "generic",
            "all",
            "false",
            "",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["findings"], 2);
        assert_eq!(v["summary"]["critical"], 1);
        assert_eq!(v["summary"]["high"], 1);
        assert_eq!(v["summary"]["statements_scanned"], 2);
        assert_eq!(v["findings"][0]["rule"], "DROP-TABLE");
        assert_eq!(v["findings"][0]["severity"], "critical");
        assert_eq!(v["findings"][0]["category"], "drop");
        assert_eq!(v["findings"][1]["rule"], "DELETE-NO-WHERE");
        assert_eq!(v["findings"][1]["statement"], 2);
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ", "generic", "all", "false", "", "text").is_err());
    }

    #[test]
    fn invalid_enums_error() {
        assert!(run("SELECT 1", "generic", "all", "false", "", "yaml").is_err());
        assert!(run("SELECT 1", "cobol", "all", "false", "", "text").is_err());
        assert!(run("SELECT 1", "generic", "urgent", "false", "", "text").is_err());
        assert!(run("SELECT 1", "generic", "all", "maybe", "", "text").is_err());
        assert!(run("SELECT 1", "generic", "all", "false", "vacuum", "text").is_err());
    }
}
