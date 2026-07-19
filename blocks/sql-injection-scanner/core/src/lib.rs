//! sql-injection-scanner core — pure compute, shared by the chat skill block and the web page.
//!
//! A static heuristic scanner. It reads a code snippet line by line and flags the
//! places where a SQL statement is assembled from a variable instead of being
//! parameterized — string concatenation (`"… " + name`, PHP `. $name`), string
//! interpolation (Python f-strings, JS/TS template literals `${…}`, C# `$"…"`,
//! Ruby `#{…}`), and format/printf building (`"…".format()`, `"…" % v`,
//! `sprintf`/`Sprintf`/`String.Format`). It also warns when a bare variable is
//! handed to an `execute()`/`query()` call. It never runs the code or touches a
//! database — it only pattern-matches, so it can miss cleverly split queries and
//! can occasionally flag a false positive. No wafer/wasm-bindgen deps.

use regex::Regex;
use serde_json::json;

/// Severity of a finding. `High` = a variable is provably joined into SQL text;
/// `Medium` = a variable is executed as SQL and its construction can't be seen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
        }
    }
}

/// Minimum severity to include in the report.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MinSeverity {
    All,
    High,
}

impl MinSeverity {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(MinSeverity::All),
            "high" => Ok(MinSeverity::High),
            other => Err(format!(
                "invalid min_severity {other:?}: expected \"all\" or \"high\""
            )),
        }
    }
    fn keeps(self, s: Severity) -> bool {
        match self {
            MinSeverity::All => true,
            MinSeverity::High => s == Severity::High,
        }
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
            other => Err(format!(
                "invalid format {other:?}: expected \"text\" or \"json\""
            )),
        }
    }
}

/// Language hint. `Auto` applies every syntax; a specific language restricts the
/// concatenation/interpolation patterns to that language (fewer false positives)
/// and tailors the remediation example.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    Auto,
    Python,
    Php,
    JavaScript,
    TypeScript,
    Java,
    CSharp,
    Go,
    Ruby,
}

impl Language {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Language::Auto),
            "python" | "py" => Ok(Language::Python),
            "php" => Ok(Language::Php),
            "javascript" | "js" | "node" => Ok(Language::JavaScript),
            "typescript" | "ts" => Ok(Language::TypeScript),
            "java" => Ok(Language::Java),
            "csharp" | "c#" | "cs" => Ok(Language::CSharp),
            "go" | "golang" => Ok(Language::Go),
            "ruby" | "rb" => Ok(Language::Ruby),
            other => Err(format!(
                "invalid language {other:?}: expected one of auto, python, php, javascript, \
                 typescript, java, csharp, go, ruby"
            )),
        }
    }

    fn is(self, l: Language) -> bool {
        self == Language::Auto || self == l
    }

    /// A one-line parameterized-query example for the remediation footer.
    fn fix_example(self) -> &'static str {
        match self {
            Language::Python => {
                "cursor.execute(\"SELECT * FROM users WHERE id = %s\", (user_id,))"
            }
            Language::Php => {
                "$stmt = $pdo->prepare(\"SELECT * FROM users WHERE id = :id\"); $stmt->execute(['id' => $id]);"
            }
            Language::JavaScript | Language::TypeScript => {
                "db.query(\"SELECT * FROM users WHERE id = $1\", [id]);"
            }
            Language::Java => {
                "PreparedStatement ps = conn.prepareStatement(\"SELECT * FROM users WHERE id = ?\"); ps.setInt(1, id);"
            }
            Language::CSharp => {
                "cmd.CommandText = \"SELECT * FROM users WHERE id = @id\"; cmd.Parameters.AddWithValue(\"@id\", id);"
            }
            Language::Go => "db.Query(\"SELECT * FROM users WHERE id = $1\", id)",
            Language::Ruby => "db.exec_params(\"SELECT * FROM users WHERE id = $1\", [id])",
            Language::Auto => "",
        }
    }
}

/// One detected issue.
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub rule: &'static str,
    pub message: &'static str,
    pub snippet: String,
}

/// Entry point: scan `code` and render the report in `format`, filtered to
/// `min_severity`, with syntax patterns and the fix example tuned by `language`.
pub fn run(code: &str, language: &str, min_severity: &str, format: &str) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("no code provided".into());
    }
    let lang = Language::parse(language)?;
    let min = MinSeverity::parse(min_severity)?;
    let fmt = Format::parse(format)?;

    let lines_scanned = code.lines().count();
    let findings: Vec<Finding> = scan(code, lang)
        .into_iter()
        .filter(|f| min.keeps(f.severity))
        .collect();

    Ok(match fmt {
        Format::Text => render_text(&findings, lines_scanned, lang),
        Format::Json => render_json(&findings, lines_scanned),
    })
}

// SQL command / clause keywords that mark a line as "SQL-ish".
const SQL_KEYWORD: &str = r"(?i)\b(select|insert\s+into|update|delete|replace|merge|drop\s+table|truncate|alter\s+table|create\s+table|from|where|values|order\s+by|group\s+by|join|union)\b";

/// Does this line contain a quoted string AND a SQL keyword (so a dynamic-build
/// match on it is very likely building an actual query)?
fn line_has_sql(line: &str, kw: &Regex) -> bool {
    (line.contains('"') || line.contains('\'') || line.contains('`')) && kw.is_match(line)
}

fn col_of(line: &str, byte_start: usize) -> usize {
    line.get(..byte_start)
        .map(|p| p.chars().count() + 1)
        .unwrap_or(1)
}

/// A dynamic-construction detector: a rule + a regex + which languages enable it.
struct Detector {
    rule: &'static str,
    message: &'static str,
    severity: Severity,
    re: Regex,
    enabled: bool,
}

fn scan(code: &str, lang: Language) -> Vec<Finding> {
    let kw = Regex::new(SQL_KEYWORD).unwrap();

    let concat_plus = lang.is(Language::Python)
        || lang.is(Language::JavaScript)
        || lang.is(Language::TypeScript)
        || lang.is(Language::Java)
        || lang.is(Language::CSharp);

    // Order matters: the first matching HIGH detector wins for a given line.
    let detectors = vec![
        // --- concatenation (HIGH) ---
        Detector {
            rule: "SQLI-CONCAT",
            message:
                "SQL query built with string concatenation; a variable is joined directly into the SQL text.",
            severity: Severity::High,
            // "…" + var   OR   var + "…"   (also single quotes / backticks)
            re: Regex::new(r#"(?:["'`]\s*\+\s*[A-Za-z_$@])|(?:[A-Za-z_$@]\w*\s*\+\s*["'`])"#)
                .unwrap(),
            enabled: concat_plus,
        },
        Detector {
            rule: "SQLI-CONCAT",
            message:
                "SQL query built with string concatenation; a variable is joined directly into the SQL text.",
            severity: Severity::High,
            // PHP "." concatenation with a $variable
            re: Regex::new(r#"(?:["']\s*\.\s*\$)|(?:\$\w+\s*\.\s*["'])"#).unwrap(),
            enabled: lang.is(Language::Php),
        },
        // --- interpolation (HIGH) ---
        Detector {
            rule: "SQLI-INTERP",
            message:
                "SQL query built by string interpolation; a variable is substituted straight into the SQL text.",
            severity: Severity::High,
            // Python f-string with a {expr}
            re: Regex::new(r#"\bf["'][^"']*\{[^}]"#).unwrap(),
            enabled: lang.is(Language::Python),
        },
        Detector {
            rule: "SQLI-INTERP",
            message:
                "SQL query built by string interpolation; a variable is substituted straight into the SQL text.",
            severity: Severity::High,
            // JS/TS template literal with ${expr}
            re: Regex::new(r"`[^`]*\$\{[^}]").unwrap(),
            enabled: lang.is(Language::JavaScript) || lang.is(Language::TypeScript),
        },
        Detector {
            rule: "SQLI-INTERP",
            message:
                "SQL query built by string interpolation; a variable is substituted straight into the SQL text.",
            severity: Severity::High,
            // C# interpolated string $"…{expr}…"
            re: Regex::new(r#"\$"[^"]*\{[^}]"#).unwrap(),
            enabled: lang.is(Language::CSharp),
        },
        Detector {
            rule: "SQLI-INTERP",
            message:
                "SQL query built by string interpolation; a variable is substituted straight into the SQL text.",
            severity: Severity::High,
            // Ruby "#{expr}" interpolation
            re: Regex::new(r#""[^"]*#\{[^}]"#).unwrap(),
            enabled: lang.is(Language::Ruby),
        },
        // --- format / printf (HIGH) ---
        Detector {
            rule: "SQLI-FORMAT",
            message:
                "SQL query built with a format/printf call; a variable is formatted into the SQL text.",
            severity: Severity::High,
            // "…".format(  or  "…" % var
            re: Regex::new(r#"(?:["']\s*\.\s*format\s*\()|(?:["']\s*%\s*[\(A-Za-z_])"#).unwrap(),
            enabled: lang.is(Language::Python) || lang.is(Language::Java),
        },
        Detector {
            rule: "SQLI-FORMAT",
            message:
                "SQL query built with a format/printf call; a variable is formatted into the SQL text.",
            severity: Severity::High,
            // sprintf( / fmt.Sprintf( / String.Format(
            re: Regex::new(r"(?i)\b(?:sprintf|fmt\.Sprintf|String\.Format)\s*\(").unwrap(),
            enabled: lang.is(Language::Php)
                || lang.is(Language::Go)
                || lang.is(Language::CSharp)
                || lang.is(Language::Java),
        },
    ];

    // exec-of-a-bare-variable (MEDIUM) — applies to every language.
    let exec_var = Regex::new(
        r"(?i)\.(?:execute|executemany|exec|query|raw_?query|prepare)\s*\(\s*([A-Za-z_]\w*)\s*\)",
    )
    .unwrap();

    let mut findings = Vec::new();
    for (i, raw) in code.lines().enumerate() {
        let line_no = i + 1;
        let mut high_on_line = false;

        for d in &detectors {
            if !d.enabled {
                continue;
            }
            if !line_has_sql(raw, &kw) {
                continue;
            }
            if let Some(m) = d.re.find(raw) {
                findings.push(Finding {
                    line: line_no,
                    column: col_of(raw, m.start()),
                    severity: d.severity,
                    rule: d.rule,
                    message: d.message,
                    snippet: snippet_of(raw),
                });
                high_on_line = true;
                break; // one HIGH finding per line is enough
            }
        }

        if !high_on_line {
            if let Some(m) = exec_var.find(raw) {
                findings.push(Finding {
                    line: line_no,
                    column: col_of(raw, m.start()),
                    severity: Severity::Medium,
                    rule: "SQLI-EXEC-VAR",
                    message:
                        "A variable is executed as SQL; verify it was built with bound parameters, not concatenation.",
                    snippet: snippet_of(raw),
                });
            }
        }
    }
    findings
}

fn snippet_of(line: &str) -> String {
    let t = line.trim();
    if t.chars().count() > 200 {
        let cut: String = t.chars().take(200).collect();
        format!("{cut}…")
    } else {
        t.to_string()
    }
}

fn counts(findings: &[Finding]) -> (usize, usize) {
    let high = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let medium = findings.len() - high;
    (high, medium)
}

fn render_text(findings: &[Finding], lines_scanned: usize, lang: Language) -> String {
    if findings.is_empty() {
        return format!(
            "No injection-prone SQL construction found in {lines_scanned} line(s) scanned. \
             (Static heuristic — not a guarantee.)"
        );
    }
    let (high, medium) = counts(findings);
    let mut out = String::new();
    out.push_str(&format!(
        "{} finding(s) ({high} high, {medium} medium) in {lines_scanned} line(s) scanned\n\n",
        findings.len()
    ));
    for f in findings {
        out.push_str(&format!(
            "line {}, col {}  {}  {}  {}\n  {}\n\n",
            f.line,
            f.column,
            f.severity.tag(),
            f.rule,
            f.message,
            f.snippet
        ));
    }
    out.push_str(
        "Fix: use parameterized queries / prepared statements — pass user input as bound \
         parameters (?, %s, :name, $1), never concatenate it into the SQL string.",
    );
    let ex = lang.fix_example();
    if !ex.is_empty() {
        out.push_str(&format!("\nExample: {ex}"));
    }
    out
}

fn render_json(findings: &[Finding], lines_scanned: usize) -> String {
    let (high, medium) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "line": f.line,
                "column": f.column,
                "severity": f.severity.label(),
                "rule": f.rule,
                "message": f.message,
                "snippet": f.snippet,
            })
        })
        .collect();
    let v = json!({
        "summary": {
            "findings": findings.len(),
            "high": high,
            "medium": medium,
            "lines_scanned": lines_scanned,
        },
        "findings": items,
    });
    serde_json::to_string_pretty(&v).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_string_concatenation() {
        let code = r#"query = "SELECT * FROM users WHERE name = '" + name + "'""#;
        let out = run(code, "auto", "all", "text").unwrap();
        assert!(out.contains("SQLI-CONCAT"), "{out}");
        assert!(out.contains("1 finding(s) (1 high, 0 medium)"), "{out}");
        assert!(out.contains("line 1"), "{out}");
    }

    #[test]
    fn flags_python_fstring() {
        let code = "cursor.execute(f\"DELETE FROM logs WHERE id = {log_id}\")";
        let out = run(code, "python", "all", "text").unwrap();
        assert!(out.contains("SQLI-INTERP"), "{out}");
        assert!(out.contains("HIGH"), "{out}");
    }

    #[test]
    fn flags_percent_and_format() {
        let pct = "sql = \"SELECT * FROM t WHERE a = %s\" % val";
        assert!(run(pct, "python", "all", "text")
            .unwrap()
            .contains("SQLI-FORMAT"));
        let fmt = "q = \"SELECT * FROM t WHERE a = {}\".format(v)";
        assert!(run(fmt, "python", "all", "text")
            .unwrap()
            .contains("SQLI-FORMAT"));
    }

    #[test]
    fn flags_php_dot_concat() {
        let code = "$sql = \"SELECT * FROM users WHERE id = \" . $id;";
        let out = run(code, "php", "all", "text").unwrap();
        assert!(out.contains("SQLI-CONCAT"), "{out}");
    }

    #[test]
    fn flags_template_literal() {
        let code = "db.query(`SELECT * FROM users WHERE id = ${id}`)";
        let out = run(code, "javascript", "all", "text").unwrap();
        assert!(out.contains("SQLI-INTERP"), "{out}");
    }

    #[test]
    fn exec_of_bare_variable_is_medium() {
        let code = "cursor.execute(sql)";
        let out = run(code, "auto", "all", "text").unwrap();
        assert!(out.contains("SQLI-EXEC-VAR"), "{out}");
        assert!(out.contains("MEDIUM"), "{out}");
    }

    #[test]
    fn parameterized_query_is_clean() {
        let code = "cursor.execute(\"SELECT * FROM users WHERE id = %s\", (uid,))";
        let out = run(code, "python", "all", "text").unwrap();
        assert!(out.contains("No injection-prone SQL construction"), "{out}");
    }

    #[test]
    fn min_severity_high_hides_medium() {
        let code = "cursor.execute(sql)";
        let out = run(code, "auto", "high", "text").unwrap();
        assert!(out.contains("No injection-prone SQL construction"), "{out}");
    }

    #[test]
    fn language_filter_scopes_php_dot_concat() {
        // PHP "." concatenation should NOT be flagged when language = javascript.
        let code = "$sql = \"SELECT * FROM users WHERE id = \" . $id;";
        let js = run(code, "javascript", "all", "text").unwrap();
        assert!(js.contains("No injection-prone SQL construction"), "{js}");
        let php = run(code, "php", "all", "text").unwrap();
        assert!(php.contains("SQLI-CONCAT"), "{php}");
    }

    #[test]
    fn json_output_shape() {
        let code = "query = \"SELECT * FROM t WHERE n = '\" + name + \"'\"";
        let out = run(code, "auto", "all", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["findings"], 1);
        assert_eq!(v["findings"][0]["severity"], "high");
        assert_eq!(v["findings"][0]["rule"], "SQLI-CONCAT");
        assert_eq!(v["findings"][0]["line"], 1);
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ", "auto", "all", "text").is_err());
    }

    #[test]
    fn invalid_enum_errors() {
        assert!(run("SELECT 1", "auto", "all", "yaml").is_err());
        assert!(run("SELECT 1", "cobol", "all", "text").is_err());
        assert!(run("SELECT 1", "auto", "critical", "text").is_err());
    }
}
