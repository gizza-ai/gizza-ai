//! string-literal-extractor core — extract every string literal from pasted
//! source code (single-, double-, and backtick/template-quoted, with escape
//! handling), skipping strings that appear inside comments. Pure-Rust (only
//! `serde_json` for the JSON output). No wafer/wasm-bindgen deps — shared by the
//! chat skill block and the web page.
//!
//! There is **no full language parser** here: a parser-generator's per-language
//! grammars are C libraries that neither build nor instantiate in the wasm
//! sandbox, so extraction comes from a deterministic, dependency-free quote /
//! comment tokenizer:
//!
//! - It scans the source character by character. Line comments (`//`, `#`) and
//!   block comments (`/* */`) are skipped so their contents are never mistaken
//!   for strings.
//! - Each language has a profile describing which quote characters open a
//!   *string* literal and whether `'` is a *character* literal (as in C, Java,
//!   Rust, Go, C#) — char literals are consumed so their contents don't confuse
//!   the scanner, but are not reported as strings.
//! - Escapes (`\"`, `\\`, …) inside a string prevent the quote from closing.
//!   Python triple-quoted strings, Go raw backtick strings, and Rust raw strings
//!   (`r"…"`, `r#"…"#`) are handled specially.
//!
//! The result is a list / CSV / JSON of the literals found, each with its source
//! line and column, optionally decoded, deduplicated, and length-filtered.

use serde_json::{json, Value};

/// Cap on the number of literals returned (keeps output bounded for huge inputs).
pub const MAX_LITERALS: usize = 50_000;

/// One extracted string literal.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Literal {
    /// 1-based source line of the opening quote.
    line: usize,
    /// 1-based source column (in characters) of the opening quote.
    column: usize,
    /// Quote style that opened the literal.
    quote: Quote,
    /// The literal's value: the raw text between the delimiters, or the decoded
    /// text when `decode_escapes` is set.
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    Double,
    Single,
    Backtick,
}

impl Quote {
    fn name(self) -> &'static str {
        match self {
            Quote::Double => "double",
            Quote::Single => "single",
            Quote::Backtick => "backtick",
        }
    }
    fn ch(self) -> char {
        match self {
            Quote::Double => '"',
            Quote::Single => '\'',
            Quote::Backtick => '`',
        }
    }
    fn from_char(c: char) -> Option<Quote> {
        match c {
            '"' => Some(Quote::Double),
            '\'' => Some(Quote::Single),
            '`' => Some(Quote::Backtick),
            _ => None,
        }
    }
}

/// Per-language tokenizer configuration.
struct Profile {
    /// Characters that open a *string* literal (extracted).
    str_delims: &'static [char],
    /// True if `'` is a character/rune literal — consumed but not reported.
    char_literal: bool,
    /// Line-comment prefixes.
    line_comments: &'static [&'static str],
    /// Block-comment (start, end), if any.
    block_comment: Option<(&'static str, &'static str)>,
    /// Recognize Python triple-quoted strings (`"""` / `'''`).
    triple_quotes: bool,
    /// Backtick is a *raw* string (Go): no escape processing.
    raw_backtick: bool,
    /// Recognize Rust raw strings (`r"…"`, `r#"…"#`).
    rust_raw: bool,
}

/// Resolve the public `language` value into a profile, resolving `auto`.
fn resolve_profile(language: &str, code: &str) -> Result<Profile, String> {
    let lang = language.trim().to_ascii_lowercase();
    let lang = if lang.is_empty() || lang == "auto" {
        detect_language(code)
    } else {
        lang
    };
    let p = match lang.as_str() {
        "python" | "py" => Profile {
            str_delims: &['"', '\''],
            char_literal: false,
            line_comments: &["#"],
            block_comment: None,
            triple_quotes: true,
            raw_backtick: false,
            rust_raw: false,
        },
        "javascript" | "js" | "typescript" | "ts" | "generic" => Profile {
            str_delims: &['"', '\'', '`'],
            char_literal: false,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "java" => Profile {
            str_delims: &['"'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "csharp" | "cs" | "c#" => Profile {
            str_delims: &['"'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "c" => Profile {
            str_delims: &['"'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "cpp" | "c++" | "cxx" => Profile {
            str_delims: &['"'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "go" | "golang" => Profile {
            str_delims: &['"', '`'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: true,
            rust_raw: false,
        },
        "rust" | "rs" => Profile {
            str_delims: &['"'],
            char_literal: true,
            line_comments: &["//"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: true,
        },
        "php" => Profile {
            str_delims: &['"', '\''],
            char_literal: false,
            line_comments: &["//", "#"],
            block_comment: Some(("/*", "*/")),
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "ruby" | "rb" => Profile {
            str_delims: &['"', '\''],
            char_literal: false,
            line_comments: &["#"],
            block_comment: None,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        "shell" | "bash" | "sh" => Profile {
            str_delims: &['"', '\''],
            char_literal: false,
            line_comments: &["#"],
            block_comment: None,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
        },
        other => {
            return Err(format!(
                "unknown language '{other}' (use auto, python, javascript, typescript, java, csharp, c, cpp, go, rust, php, ruby, or shell)"
            ))
        }
    };
    Ok(p)
}

/// Auto-detect: Python (uses `#`/`def` and no `//`) vs a generic C/JS profile.
fn detect_language(code: &str) -> String {
    let has_slash_comment = code.contains("//");
    let looks_python = (code.contains("def ")
        || code.contains("elif ")
        || code.contains("\nimport ")
        || code.starts_with("import ")
        || code.contains("\nfrom ")
        || code.contains("print("))
        && !has_slash_comment;
    if looks_python {
        "python".to_string()
    } else {
        "generic".to_string()
    }
}

/// Which quote styles to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteFilter {
    All,
    Double,
    Single,
    Backtick,
}

fn parse_quote_filter(s: &str) -> Result<QuoteFilter, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(QuoteFilter::All),
        "double" | "\"" => Ok(QuoteFilter::Double),
        "single" | "'" => Ok(QuoteFilter::Single),
        "backtick" | "template" | "`" => Ok(QuoteFilter::Backtick),
        other => Err(format!(
            "unknown quotes '{other}' (use all, double, single, or backtick)"
        )),
    }
}

impl QuoteFilter {
    fn accepts(self, q: Quote) -> bool {
        match self {
            QuoteFilter::All => true,
            QuoteFilter::Double => q == Quote::Double,
            QuoteFilter::Single => q == Quote::Single,
            QuoteFilter::Backtick => q == Quote::Backtick,
        }
    }
}

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    List,
    Json,
    Csv,
}

fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "list" => Ok(Format::List),
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        other => Err(format!("unknown format '{other}' (use list, json, or csv)")),
    }
}

/// Scanner over the source characters, tracking line/column.
struct Scanner {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Scanner {
    fn new(code: &str) -> Scanner {
        Scanner {
            chars: code.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, off: usize) -> Option<char> {
        self.chars.get(self.pos + off).copied()
    }

    /// Advance one char, updating line/column.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    /// True if the upcoming chars match `s`.
    fn starts_with(&self, s: &str) -> bool {
        let mut i = self.pos;
        for want in s.chars() {
            match self.chars.get(i) {
                Some(&c) if c == want => i += 1,
                _ => return false,
            }
        }
        true
    }
}

/// Extract every string literal, returning them in the chosen format.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    code: &str,
    language: &str,
    quotes: &str,
    decode_escapes: bool,
    unique: bool,
    min_length: i64,
    format: &str,
    line_numbers: bool,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("code is empty — paste some source to extract string literals from".into());
    }
    if min_length < 0 {
        return Err("min_length must be 0 or greater".into());
    }
    let profile = resolve_profile(language, code)?;
    let filter = parse_quote_filter(quotes)?;
    let fmt = parse_format(format)?;
    let min_len = min_length as usize;

    let mut literals = scan(code, &profile, decode_escapes);

    // Filter by quote style and minimum length.
    literals.retain(|l| filter.accepts(l.quote) && l.value.chars().count() >= min_len);

    // Deduplicate by value (keep first occurrence) if requested.
    let mut truncated = false;
    if unique {
        let mut seen = std::collections::HashSet::new();
        literals.retain(|l| seen.insert(l.value.clone()));
    }
    if literals.len() > MAX_LITERALS {
        literals.truncate(MAX_LITERALS);
        truncated = true;
    }

    Ok(render(&literals, fmt, line_numbers, truncated))
}

/// Scan the source and collect every string literal.
fn scan(code: &str, p: &Profile, decode: bool) -> Vec<Literal> {
    let mut s = Scanner::new(code);
    let mut out = Vec::new();

    while let Some(c) = s.peek() {
        // Block comment?
        if let Some((open, close)) = p.block_comment {
            if s.starts_with(open) {
                for _ in 0..open.chars().count() {
                    s.bump();
                }
                while s.peek().is_some() && !s.starts_with(close) {
                    s.bump();
                }
                for _ in 0..close.chars().count() {
                    s.bump();
                }
                continue;
            }
        }
        // Line comment?
        let mut matched_line_comment = false;
        for lc in p.line_comments {
            if s.starts_with(lc) {
                while let Some(ch) = s.peek() {
                    if ch == '\n' {
                        break;
                    }
                    s.bump();
                }
                matched_line_comment = true;
                break;
            }
        }
        if matched_line_comment {
            continue;
        }
        // Rust raw string: r"…" or r#…"…"#…
        if p.rust_raw && c == 'r' {
            if let Some(next) = s.peek_at(1) {
                if next == '"' || next == '#' {
                    if try_rust_raw(&mut s, &mut out) {
                        continue;
                    }
                }
            }
        }
        // A string or char delimiter?
        if let Some(q) = Quote::from_char(c) {
            let is_str = p.str_delims.contains(&c);
            let is_char = c == '\'' && p.char_literal;
            if is_str {
                // Python triple-quote?
                if p.triple_quotes
                    && (c == '"' || c == '\'')
                    && s.peek_at(1) == Some(c)
                    && s.peek_at(2) == Some(c)
                {
                    scan_triple(&mut s, q, decode, &mut out);
                    continue;
                }
                let raw_backtick = c == '`' && p.raw_backtick;
                scan_string(&mut s, q, decode, raw_backtick, &mut out);
                continue;
            } else if is_char {
                // Consume the char literal so its contents don't confuse us;
                // don't report it (not a string literal).
                scan_char_literal(&mut s);
                continue;
            }
        }
        s.bump();
    }
    out
}

/// Scan a normal (single-line-ish) string starting at the opening delimiter.
/// A bare newline ends a non-backtick string (treated as unterminated, forgiving).
fn scan_string(s: &mut Scanner, q: Quote, decode: bool, raw: bool, out: &mut Vec<Literal>) {
    let line = s.line;
    let column = s.col;
    let delim = q.ch();
    s.bump(); // opening quote
    let allow_newline = q == Quote::Backtick;
    let mut raw_text = String::new();
    while let Some(c) = s.peek() {
        if c == delim {
            s.bump(); // closing quote
            break;
        }
        if c == '\n' && !allow_newline {
            break; // unterminated string; stop at end of line
        }
        if c == '\\' && !raw {
            // Include the backslash and the escaped char verbatim in raw_text.
            raw_text.push('\\');
            s.bump();
            if let Some(esc) = s.peek() {
                raw_text.push(esc);
                s.bump();
            }
            continue;
        }
        raw_text.push(c);
        s.bump();
    }
    let value = if decode && !raw {
        decode_escapes_str(&raw_text)
    } else {
        raw_text
    };
    out.push(Literal {
        line,
        column,
        quote: q,
        value,
    });
}

/// Scan a Python triple-quoted string (`"""…"""` / `'''…'''`).
fn scan_triple(s: &mut Scanner, q: Quote, decode: bool, out: &mut Vec<Literal>) {
    let line = s.line;
    let column = s.col;
    let delim = q.ch();
    let triple: String = std::iter::repeat(delim).take(3).collect();
    for _ in 0..3 {
        s.bump(); // opening triple
    }
    let mut raw_text = String::new();
    while s.peek().is_some() {
        if s.starts_with(&triple) {
            for _ in 0..3 {
                s.bump();
            }
            break;
        }
        if s.peek() == Some('\\') {
            raw_text.push('\\');
            s.bump();
            if let Some(esc) = s.peek() {
                raw_text.push(esc);
                s.bump();
            }
            continue;
        }
        if let Some(c) = s.bump() {
            raw_text.push(c);
        }
    }
    let value = if decode {
        decode_escapes_str(&raw_text)
    } else {
        raw_text
    };
    out.push(Literal {
        line,
        column,
        quote: q,
        value,
    });
}

/// Try to scan a Rust raw string at the current position. Returns true if one
/// was consumed. Raw strings never process escapes.
fn try_rust_raw(s: &mut Scanner, out: &mut Vec<Literal>) -> bool {
    let line = s.line;
    let column = s.col;
    // Look ahead: 'r' then zero+ '#' then '"'.
    let mut off = 1;
    let mut hashes = 0;
    while s.peek_at(off) == Some('#') {
        hashes += 1;
        off += 1;
    }
    if s.peek_at(off) != Some('"') {
        return false;
    }
    // Commit: consume 'r', the hashes, and the opening quote.
    s.bump(); // r
    for _ in 0..hashes {
        s.bump();
    }
    s.bump(); // opening quote
    let closing: String = {
        let mut t = String::from("\"");
        for _ in 0..hashes {
            t.push('#');
        }
        t
    };
    let mut raw_text = String::new();
    while s.peek().is_some() {
        if s.starts_with(&closing) {
            for _ in 0..closing.chars().count() {
                s.bump();
            }
            break;
        }
        if let Some(c) = s.bump() {
            raw_text.push(c);
        }
    }
    out.push(Literal {
        line,
        column,
        quote: Quote::Double,
        value: raw_text,
    });
    true
}

/// Consume a `'…'` char/rune literal without reporting it.
fn scan_char_literal(s: &mut Scanner) {
    s.bump(); // opening '
    while let Some(c) = s.peek() {
        if c == '\n' {
            break;
        }
        if c == '\\' {
            s.bump();
            s.bump();
            continue;
        }
        if c == '\'' {
            s.bump();
            break;
        }
        s.bump();
    }
}

/// Decode common escape sequences into their actual characters.
fn decode_escapes_str(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let e = chars[i + 1];
        i += 2;
        match e {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '0' => out.push('\0'),
            'b' => out.push('\u{08}'),
            'f' => out.push('\u{0C}'),
            'v' => out.push('\u{0B}'),
            'a' => out.push('\u{07}'),
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            '`' => out.push('`'),
            '\n' => {} // line continuation
            'x' => {
                let (val, used) = read_hex(&chars, i, 2);
                if let Some(ch) = val {
                    out.push(ch);
                    i += used;
                } else {
                    out.push('\\');
                    out.push('x');
                }
            }
            'u' => {
                if chars.get(i) == Some(&'{') {
                    // \u{H+}
                    let mut j = i + 1;
                    let mut hex = String::new();
                    while j < chars.len() && chars[j] != '}' {
                        hex.push(chars[j]);
                        j += 1;
                    }
                    if j < chars.len() && !hex.is_empty() {
                        if let Some(ch) =
                            u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                        {
                            out.push(ch);
                            i = j + 1;
                            continue;
                        }
                    }
                    out.push('\\');
                    out.push('u');
                } else {
                    let (val, used) = read_hex(&chars, i, 4);
                    if let Some(ch) = val {
                        out.push(ch);
                        i += used;
                    } else {
                        out.push('\\');
                        out.push('u');
                    }
                }
            }
            'U' => {
                let (val, used) = read_hex(&chars, i, 8);
                if let Some(ch) = val {
                    out.push(ch);
                    i += used;
                } else {
                    out.push('\\');
                    out.push('U');
                }
            }
            other => {
                // Unknown escape: keep it literal (backslash + char).
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// Read exactly `n` hex digits starting at `start`; return the decoded char and
/// how many chars were consumed, or (None, 0) if fewer than `n` digits.
fn read_hex(chars: &[char], start: usize, n: usize) -> (Option<char>, usize) {
    if start + n > chars.len() {
        return (None, 0);
    }
    let mut hex = String::new();
    for k in 0..n {
        let d = chars[start + k];
        if d.is_ascii_hexdigit() {
            hex.push(d);
        } else {
            return (None, 0);
        }
    }
    match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
        Some(ch) => (Some(ch), n),
        None => (None, 0),
    }
}

/// Escape control characters for safe single-line display in list output.
fn display_safe(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out
}

/// Render the collected literals in the chosen format.
fn render(literals: &[Literal], fmt: Format, line_numbers: bool, truncated: bool) -> String {
    match fmt {
        Format::List => {
            if literals.is_empty() {
                return "No string literals found.".to_string();
            }
            let mut lines: Vec<String> = literals
                .iter()
                .map(|l| {
                    let safe = display_safe(&l.value);
                    if line_numbers {
                        format!("{safe}  [L{}]", l.line)
                    } else {
                        safe
                    }
                })
                .collect();
            if truncated {
                lines.push(format!("… (truncated at {MAX_LITERALS} literals)"));
            }
            lines.join("\n")
        }
        Format::Csv => {
            let mut rows = vec!["line,column,quote,value".to_string()];
            for l in literals {
                rows.push(format!(
                    "{},{},{},{}",
                    l.line,
                    l.column,
                    l.quote.name(),
                    csv_field(&l.value)
                ));
            }
            if truncated {
                rows.push(format!("# truncated at {MAX_LITERALS} literals"));
            }
            rows.join("\n")
        }
        Format::Json => {
            let arr: Vec<Value> = literals
                .iter()
                .map(|l| {
                    json!({
                        "line": l.line,
                        "column": l.column,
                        "quote": l.quote.name(),
                        "value": l.value,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| "[]".to_string())
        }
    }
}

/// CSV-quote a field: wrap in double quotes and double any internal quote.
fn csv_field(v: &str) -> String {
    let escaped = v.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_extracts_js_strings_with_line_numbers() {
        let code =
            "const a = \"hello\";\n// \"in a comment\"\nconst b = 'world';\nconst c = `tpl`;\n";
        let out = extract(code, "javascript", "all", false, false, 0, "list", true).unwrap();
        assert_eq!(out, "hello  [L1]\nworld  [L3]\ntpl  [L4]");
    }

    #[test]
    fn error_on_empty_code() {
        let err = extract("   \n  ", "auto", "all", false, false, 0, "list", true).unwrap_err();
        assert!(err.contains("code is empty"), "got: {err}");
    }

    #[test]
    fn error_on_unknown_language() {
        let err =
            extract("x = \"a\"", "cobol", "all", false, false, 0, "list", true).unwrap_err();
        assert!(err.contains("unknown language"), "got: {err}");
    }

    #[test]
    fn ignores_strings_in_block_and_line_comments() {
        let code = "int x = 0; /* \"blockstr\" */ char* s = \"kept\"; // \"linestr\"\n";
        let out = extract(code, "c", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "kept");
    }

    #[test]
    fn c_single_quote_is_char_not_string() {
        // 'x' is a char literal; only the double-quoted string is extracted.
        let code = "char c = 'x'; const char* s = \"str\";";
        let out = extract(code, "c", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "str");
    }

    #[test]
    fn quotes_filter_double_only() {
        let code = "a = \"dq\"\nb = 'sq'\n";
        let out = extract(code, "python", "double", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "dq");
    }

    #[test]
    fn escaped_quote_does_not_close_string() {
        let code = "s = \"a\\\"b\"";
        let out = extract(code, "javascript", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "a\\\"b");
    }

    #[test]
    fn decode_escapes_interprets_sequences() {
        let code = "s = \"tab\\there\\u0041\"";
        let out = extract(code, "javascript", "all", true, false, 0, "list", false).unwrap();
        // \t decoded then re-escaped for display; A -> 'A'.
        assert_eq!(out, "tab\\thereA");
    }

    #[test]
    fn python_triple_quoted_multiline() {
        let code = "x = \"\"\"line1\nline2\"\"\"\n";
        let out = extract(code, "python", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "line1\\nline2");
    }

    #[test]
    fn go_raw_backtick_keeps_backslashes() {
        let code = "s := `raw\\nstring`";
        let out = extract(code, "go", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "raw\\nstring");
    }

    #[test]
    fn rust_raw_string_with_hashes() {
        let code = "let s = r#\"has \" quote\"#;";
        let out = extract(code, "rust", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "has \" quote");
    }

    #[test]
    fn unique_dedups_and_min_length_filters() {
        let code = "a=\"hi\"\nb=\"hi\"\nc=\"x\"\nd=\"longer\"\n";
        let out = extract(code, "python", "all", false, true, 2, "list", false).unwrap();
        assert_eq!(out, "hi\nlonger");
    }

    #[test]
    fn json_format_has_positions() {
        let code = "x = \"hi\"";
        let out = extract(code, "python", "all", false, false, 0, "json", false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["value"], "hi");
        assert_eq!(v[0]["line"], 1);
        assert_eq!(v[0]["column"], 5);
        assert_eq!(v[0]["quote"], "double");
    }

    #[test]
    fn csv_format_quotes_values() {
        let code = "x = \"a,b\"";
        let out = extract(code, "python", "all", false, false, 0, "csv", false).unwrap();
        assert_eq!(out, "line,column,quote,value\n1,5,double,\"a,b\"");
    }

    #[test]
    fn auto_detects_python() {
        let code = "def greet():\n    return \"hi\"\n";
        let out = extract(code, "auto", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "hi");
    }

    #[test]
    fn empty_result_message() {
        let code = "let x = 5; // no strings here\n";
        let out = extract(code, "rust", "all", false, false, 0, "list", false).unwrap();
        assert_eq!(out, "No string literals found.");
    }
}
