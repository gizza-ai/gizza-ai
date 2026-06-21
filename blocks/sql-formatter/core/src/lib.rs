//! gizza-ai/sql-formatter core — pretty-print / standardize a SQL query.
//!
//! A dependency-free SQL tokenizer + formatter. It splits a query into tokens
//! (string/identifier literals, line & block comments, multi-char operators,
//! punctuation, words) and re-emits them with consistent line breaks and
//! indentation around the major clauses, keyword case normalization, and a
//! configurable indent width. It is deliberately forgiving — it never rejects a
//! query, it just reformats the token stream — so it works on any SQL dialect.

/// Keyword casing options.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeywordCase {
    Upper,
    Lower,
    Preserve,
}

impl KeywordCase {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "upper" | "uppercase" => Ok(KeywordCase::Upper),
            "lower" | "lowercase" => Ok(KeywordCase::Lower),
            "preserve" | "keep" | "original" => Ok(KeywordCase::Preserve),
            other => Err(format!(
                "invalid keyword_case '{other}' (expected upper, lower, or preserve)"
            )),
        }
    }
}

/// SQL reserved words we recognize for casing and layout decisions.
const KEYWORDS: &[&str] = &[
    "select", "from", "where", "and", "or", "not", "as", "on", "in", "is", "null", "like",
    "between", "join", "inner", "left", "right", "full", "outer", "cross", "group", "by",
    "order", "having", "limit", "offset", "fetch", "union", "all", "distinct", "insert", "into",
    "values", "update", "set", "delete", "create", "table", "alter", "drop", "view", "index",
    "primary", "key", "foreign", "references", "default", "constraint", "unique", "check",
    "with", "case", "when", "then", "else", "end", "exists", "asc", "desc", "using", "natural",
    "true", "false", "begin", "commit", "rollback", "grant", "revoke", "if", "for", "while",
    "return", "returning", "over", "partition", "window", "cast", "count", "sum", "avg", "min",
    "max", "coalesce", "nullif", "intersect", "except", "add", "column", "rename", "to",
];

/// Clause keywords that start a new top-level line.
const NEWLINE_BEFORE: &[&str] = &[
    "select", "from", "where", "group", "order", "having", "limit", "offset", "union",
    "intersect", "except", "values", "set", "into",
];

/// Keywords that begin a JOIN line (any leading join modifier).
const JOIN_LEADERS: &[&str] = &["join", "inner", "left", "right", "full", "cross", "natural"];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Punct(char),
    Op(String),
    Str(String),   // quoted literal incl. quotes
    Ident(String), // backtick / bracket / double-quote delimited identifier incl. delimiters
    LineComment(String),
    BlockComment(String),
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '#'
}

/// Tokenize the SQL source. Forgiving: unterminated strings/comments run to EOF.
fn tokenize(sql: &str) -> Vec<Tok> {
    let b: Vec<char> = sql.chars().collect();
    let n = b.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < n {
        let c = b[i];
        // whitespace — collapse (layout is rebuilt later)
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // line comment -- ...
        if c == '-' && i + 1 < n && b[i + 1] == '-' {
            let start = i;
            while i < n && b[i] != '\n' {
                i += 1;
            }
            out.push(Tok::LineComment(b[start..i].iter().collect()));
            continue;
        }
        // block comment /* ... */
        if c == '/' && i + 1 < n && b[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push(Tok::BlockComment(b[start..i].iter().collect()));
            continue;
        }
        // string literal ' ... ' with '' escape
        if c == '\'' {
            let start = i;
            i += 1;
            while i < n {
                if b[i] == '\'' {
                    if i + 1 < n && b[i + 1] == '\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Tok::Str(b[start..i].iter().collect()));
            continue;
        }
        // quoted identifier: "..." or `...`
        if c == '"' || c == '`' {
            let close = c;
            let start = i;
            i += 1;
            while i < n {
                if b[i] == close {
                    if i + 1 < n && b[i + 1] == close {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Tok::Ident(b[start..i].iter().collect()));
            continue;
        }
        // bracket identifier [...]
        if c == '[' {
            let start = i;
            i += 1;
            while i < n && b[i] != ']' {
                i += 1;
            }
            i = (i + 1).min(n);
            out.push(Tok::Ident(b[start..i].iter().collect()));
            continue;
        }
        // multi-char operators
        let two: String = if i + 1 < n {
            b[i..i + 2].iter().collect()
        } else {
            String::new()
        };
        if matches!(two.as_str(), "<=" | ">=" | "<>" | "!=" | "||" | "::" | ":=") {
            out.push(Tok::Op(two));
            i += 2;
            continue;
        }
        // punctuation
        if "(),;".contains(c) {
            out.push(Tok::Punct(c));
            i += 1;
            continue;
        }
        // single-char operators
        if "+-*/%=<>.".contains(c) {
            out.push(Tok::Op(c.to_string()));
            i += 1;
            continue;
        }
        // word (identifier / keyword / number)
        if is_word_char(c) {
            let start = i;
            while i < n && is_word_char(b[i]) {
                i += 1;
            }
            out.push(Tok::Word(b[start..i].iter().collect()));
            continue;
        }
        // anything else (e.g. ? @ :) — emit as op so nothing is lost
        out.push(Tok::Op(c.to_string()));
        i += 1;
    }
    out
}

fn is_keyword(w: &str) -> bool {
    let lw = w.to_ascii_lowercase();
    KEYWORDS.contains(&lw.as_str())
}

fn cased(w: &str, case: KeywordCase) -> String {
    if !is_keyword(w) {
        return w.to_string();
    }
    match case {
        KeywordCase::Upper => w.to_ascii_uppercase(),
        KeywordCase::Lower => w.to_ascii_lowercase(),
        KeywordCase::Preserve => w.to_string(),
    }
}

fn tok_text(t: &Tok, case: KeywordCase) -> String {
    match t {
        Tok::Word(w) => cased(w, case),
        Tok::Punct(c) => c.to_string(),
        Tok::Op(o) => o.clone(),
        Tok::Str(s) => s.clone(),
        Tok::Ident(s) => s.clone(),
        Tok::LineComment(s) => s.clone(),
        Tok::BlockComment(s) => s.clone(),
    }
}

fn lower_word(t: &Tok) -> Option<String> {
    if let Tok::Word(w) = t {
        Some(w.to_ascii_lowercase())
    } else {
        None
    }
}

/// Pretty-print / standardize a SQL query.
///
/// `indent` = spaces per nesting level (clamped 0..=8). `case` controls keyword casing.
pub fn format(sql: &str, indent: usize, case: KeywordCase) -> Result<String, String> {
    if sql.trim().is_empty() {
        return Err("empty SQL input".to_string());
    }
    let indent = indent.min(8);
    let toks = tokenize(sql);
    let unit = " ".repeat(indent);

    let mut out = String::new();
    let mut line = String::new();
    let mut depth: usize = 0; // paren nesting for indentation

    macro_rules! flush_line {
        () => {{
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            }
            line.clear();
        }};
    }
    macro_rules! newline_indent {
        ($d:expr) => {{
            flush_line!();
            for _ in 0..$d {
                line.push_str(&unit);
            }
        }};
    }

    let mut idx = 0;
    while idx < toks.len() {
        let t = &toks[idx];
        let lw = lower_word(t);

        // Statement terminator: ';' then a blank line between statements.
        if let Tok::Punct(';') = t {
            let trimmed = line.trim_end().to_string();
            line = format!("{trimmed};");
            flush_line!();
            out.push('\n');
            depth = 0;
            idx += 1;
            continue;
        }

        // Opening paren — keep inline, bump depth.
        if let Tok::Punct('(') = t {
            if needs_space_before(&line, '(') {
                line.push(' ');
            }
            line.push('(');
            depth += 1;
            idx += 1;
            continue;
        }
        if let Tok::Punct(')') = t {
            depth = depth.saturating_sub(1);
            line = line.trim_end().to_string();
            line.push(')');
            idx += 1;
            continue;
        }
        if let Tok::Punct(',') = t {
            line = line.trim_end().to_string();
            line.push(',');
            // Newline after commas at top level (SELECT / column lists); inline inside parens.
            if depth == 0 {
                newline_indent!(1);
            } else {
                line.push(' ');
            }
            idx += 1;
            continue;
        }

        // Line comment: append, then break to a fresh indented line.
        if let Tok::LineComment(_) = t {
            if !line.trim().is_empty() {
                line.push(' ');
            }
            line.push_str(&tok_text(t, case));
            newline_indent!(depth);
            idx += 1;
            continue;
        }

        // Clause / join / boolean line breaks (only at top paren depth).
        if let Some(ref w) = lw {
            if depth == 0 && NEWLINE_BEFORE.contains(&w.as_str()) && !out_is_empty(&out, &line) {
                newline_indent!(0);
            } else if depth == 0 && JOIN_LEADERS.contains(&w.as_str()) {
                let prev = if idx > 0 { lower_word(&toks[idx - 1]) } else { None };
                let prev_is_join_mod = prev
                    .as_deref()
                    .map(|p| JOIN_LEADERS.contains(&p) || p == "outer")
                    .unwrap_or(false);
                if !prev_is_join_mod && !out_is_empty(&out, &line) {
                    newline_indent!(0);
                }
            } else if depth == 0 && (w == "and" || w == "or") && !out_is_empty(&out, &line) {
                newline_indent!(1);
            }
        }

        // Emit the token with appropriate spacing.
        let text = tok_text(t, case);
        let c0 = text.chars().next().unwrap_or(' ');
        let at_line_start = line.trim_start().is_empty();
        if !line.is_empty() && !at_line_start && needs_space_before(&line, c0) {
            line.push(' ');
        }
        line.push_str(&text);
        idx += 1;
    }
    flush_line!();

    let result = out.trim_end().to_string();
    if result.is_empty() {
        return Err("empty SQL input".to_string());
    }
    Ok(result)
}

fn out_is_empty(out: &str, line: &str) -> bool {
    out.trim().is_empty() && line.trim().is_empty()
}

/// Decide whether a space should precede a token whose first char is `next`,
/// given the text already on the current line.
fn needs_space_before(line: &str, next: char) -> bool {
    let last = match line.chars().last() {
        Some(c) => c,
        None => return false,
    };
    if last == '(' {
        return false;
    }
    if matches!(next, ')' | ',' | ';' | '.') {
        return false;
    }
    if last == '.' {
        return false;
    }
    // function call: word/identifier immediately followed by '(' => no space
    if next == '('
        && (last.is_ascii_alphanumeric()
            || last == '_'
            || last == '`'
            || last == '"'
            || last == ']')
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_select_uppercased() {
        let out = format(
            "select id, name from users where age>18 order by name",
            2,
            KeywordCase::Upper,
        )
        .unwrap();
        assert!(out.contains("SELECT"));
        assert!(out.contains("\nFROM users"));
        assert!(out.contains("\nWHERE age > 18"));
        assert!(out.contains("\nORDER BY name"));
        assert!(out.contains("SELECT id,\n  name"), "got:\n{out}");
    }

    #[test]
    fn lowercase_keywords() {
        let out = format("SELECT A FROM T", 2, KeywordCase::Lower).unwrap();
        assert!(out.starts_with("select A"));
        assert!(out.contains("from T"));
    }

    #[test]
    fn preserve_keeps_case() {
        let out = format("Select x From y", 2, KeywordCase::Preserve).unwrap();
        assert!(out.contains("Select x"));
        assert!(out.contains("From y"));
    }

    #[test]
    fn join_on_new_line() {
        let out = format(
            "select * from a inner join b on a.id = b.id",
            2,
            KeywordCase::Upper,
        )
        .unwrap();
        assert!(out.contains("\nINNER JOIN b ON a.id = b.id"), "got:\n{out}");
    }

    #[test]
    fn and_or_break() {
        let out = format(
            "select * from t where a = 1 and b = 2 or c = 3",
            2,
            KeywordCase::Upper,
        )
        .unwrap();
        assert!(
            out.contains("WHERE a = 1\n  AND b = 2\n  OR c = 3"),
            "got:\n{out}"
        );
    }

    #[test]
    fn preserves_string_literal() {
        let out = format("select 'hello, world' as g from t", 2, KeywordCase::Upper).unwrap();
        assert!(out.contains("'hello, world'"), "got:\n{out}");
    }

    #[test]
    fn function_call_no_space() {
        let out = format("select count(*) from t", 2, KeywordCase::Upper).unwrap();
        assert!(out.contains("COUNT(*)"), "got:\n{out}");
    }

    #[test]
    fn block_comment_inline() {
        let out = format("select /* hi */ a from t", 2, KeywordCase::Upper).unwrap();
        assert!(out.contains("/* hi */"), "got:\n{out}");
    }

    #[test]
    fn indent_width_respected() {
        let out = format("select a, b from t", 4, KeywordCase::Upper).unwrap();
        assert!(out.contains("SELECT a,\n    b"), "got:\n{out}");
    }

    #[test]
    fn empty_is_error() {
        assert!(format("   ", 2, KeywordCase::Upper).is_err());
    }

    #[test]
    fn case_parse() {
        assert!(KeywordCase::parse("UPPER").is_ok());
        assert!(KeywordCase::parse("lower").is_ok());
        assert!(KeywordCase::parse("preserve").is_ok());
        assert!(KeywordCase::parse("sideways").is_err());
    }

    #[test]
    fn two_statements_blank_separated() {
        let out = format("select 1; select 2;", 2, KeywordCase::Upper).unwrap();
        assert!(out.contains("SELECT 1;"), "got:\n{out}");
        assert!(out.contains("SELECT 2;"), "got:\n{out}");
    }
}
