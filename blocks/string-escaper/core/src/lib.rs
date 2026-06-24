//! string-escaper core — escape (or, where reversible, unescape) a string for a
//! chosen target syntax: JSON, JavaScript, HTML, URL, shell, SQL, or regex.
//!
//! Pure-Rust, no I/O. Each target has spec-aware escaping so the result is safe
//! to drop into that context. `unescape` is supported for the reversible targets
//! (json, javascript, html, url); shell/sql/regex escaping is one-way-only
//! (un-escaping them is ambiguous), so unescape on those returns an error.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Json,
    JavaScript,
    Html,
    Url,
    Shell,
    Sql,
    Regex,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Target::Json),
            "javascript" | "js" => Ok(Target::JavaScript),
            "html" | "xml" => Ok(Target::Html),
            "url" | "uri" => Ok(Target::Url),
            "shell" | "bash" | "sh" => Ok(Target::Shell),
            "sql" => Ok(Target::Sql),
            "regex" | "regexp" | "re" => Ok(Target::Regex),
            other => Err(format!(
                "unknown target '{other}' (use json, javascript, html, url, shell, sql, or regex)"
            )),
        }
    }

    /// Whether this target can be un-escaped (reversed).
    pub fn reversible(self) -> bool {
        matches!(
            self,
            Target::Json | Target::JavaScript | Target::Html | Target::Url
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Escape,
    Unescape,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "escape" | "encode" | "" => Ok(Mode::Escape),
            "unescape" | "decode" => Ok(Mode::Unescape),
            other => Err(format!("unknown mode '{other}' (use escape or unescape)")),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn escape_json(text: &str, quotes: bool) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    if quotes {
        quoted
    } else {
        quoted[1..quoted.len() - 1].to_string()
    }
}

fn unescape_json(text: &str) -> Result<String, String> {
    let t = text.trim();
    let wrapped = if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t.to_string()
    } else {
        format!("\"{text}\"")
    };
    serde_json::from_str::<String>(&wrapped)
        .map_err(|e| format!("invalid JSON string escaping: {e}"))
}

// ---------------------------------------------------------------------------
// JavaScript string literal (single/double quotes + backtick + control chars)
// ---------------------------------------------------------------------------

fn escape_js(text: &str, quotes: bool) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    if quotes {
        out.push('"');
    }
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '`' => out.push_str("\\`"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{000B}' => out.push_str("\\v"),
            '\0' => out.push_str("\\0"),
            // U+2028 / U+2029 break JS string literals (not allowed raw)
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    if quotes {
        out.push('"');
    }
    out
}

fn unescape_js(text: &str) -> Result<String, String> {
    let mut t = text.trim();
    // strip a single matching pair of surrounding quotes
    if t.len() >= 2 {
        let b = t.as_bytes();
        let first = b[0];
        let last = b[t.len() - 1];
        if (first == b'"' || first == b'\'' || first == b'`') && first == last {
            t = &t[1..t.len() - 1];
        }
    }
    let mut out = String::with_capacity(t.len());
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let e = chars
            .next()
            .ok_or_else(|| "dangling backslash at end of input".to_string())?;
        match e {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'v' => out.push('\u{000B}'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '`' => out.push('`'),
            'x' => {
                let h: String = (0..2).filter_map(|_| chars.next()).collect();
                let code = u32::from_str_radix(&h, 16)
                    .map_err(|_| format!("invalid \\x escape '\\x{h}'"))?;
                out.push(
                    char::from_u32(code)
                        .ok_or_else(|| format!("invalid code point U+{code:04X}"))?,
                );
            }
            'u' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    let h: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    let code = u32::from_str_radix(&h, 16)
                        .map_err(|_| format!("invalid \\u{{}} escape '\\u{{{h}}}'"))?;
                    out.push(
                        char::from_u32(code)
                            .ok_or_else(|| format!("invalid code point U+{code:04X}"))?,
                    );
                } else {
                    let h: String = (0..4).filter_map(|_| chars.next()).collect();
                    let code = u32::from_str_radix(&h, 16)
                        .map_err(|_| format!("invalid \\u escape '\\u{h}'"))?;
                    out.push(
                        char::from_u32(code)
                            .ok_or_else(|| format!("invalid code point U+{code:04X}"))?,
                    );
                }
            }
            other => out.push(other), // unknown escape → drop the backslash
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// HTML / XML entities
// ---------------------------------------------------------------------------

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn unescape_html(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        // gather up to the ';'
        let mut entity = String::new();
        let mut closed = false;
        while let Some(&n) = chars.peek() {
            if n == ';' {
                chars.next();
                closed = true;
                break;
            }
            if n == '&' || n.is_whitespace() || entity.len() > 32 {
                break;
            }
            entity.push(n);
            chars.next();
        }
        if !closed {
            // not a real entity — emit literally
            out.push('&');
            out.push_str(&entity);
            continue;
        }
        match entity.as_str() {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            "nbsp" => out.push('\u{00A0}'),
            other => {
                if let Some(num) = other.strip_prefix('#') {
                    let code = if let Some(hex) =
                        num.strip_prefix('x').or_else(|| num.strip_prefix('X'))
                    {
                        u32::from_str_radix(hex, 16)
                    } else {
                        num.parse::<u32>()
                    }
                    .map_err(|_| format!("invalid numeric entity '&{other};'"))?;
                    out.push(
                        char::from_u32(code)
                            .ok_or_else(|| format!("invalid code point in '&{other};'"))?,
                    );
                } else {
                    return Err(format!("unknown HTML entity '&{other};'"));
                }
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// URL (percent-encoding of a component)
// ---------------------------------------------------------------------------

fn is_url_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')
}

fn escape_url(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for &b in text.as_bytes() {
        if is_url_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unescape_url(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("truncated percent-escape".into());
                }
                let hex = &text[i + 1..i + 3];
                let v = u8::from_str_radix(hex, 16)
                    .map_err(|_| format!("invalid percent-escape '%{hex}'"))?;
                out.push(v);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "percent-decoded bytes are not valid UTF-8".into())
}

// ---------------------------------------------------------------------------
// Shell (POSIX single-quote wrapping — always safe)
// ---------------------------------------------------------------------------

fn escape_shell(text: &str) -> String {
    // Wrap in single quotes; a literal ' becomes '\''
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

// ---------------------------------------------------------------------------
// SQL (single-quote literal — double the quotes)
// ---------------------------------------------------------------------------

fn escape_sql(text: &str, quotes: bool) -> String {
    let body = text.replace('\'', "''");
    if quotes {
        format!("'{body}'")
    } else {
        body
    }
}

// ---------------------------------------------------------------------------
// Regex (escape regex metacharacters)
// ---------------------------------------------------------------------------

fn escape_regex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
            | '/' | '-' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Dispatch: escape or unescape `text` for `target`. `quotes` (escape mode only)
/// wraps the result in the target's surrounding quotes where that applies
/// (json, javascript, sql).
pub fn process(text: &str, target: Target, mode: Mode, quotes: bool) -> Result<String, String> {
    match mode {
        Mode::Escape => Ok(match target {
            Target::Json => escape_json(text, quotes),
            Target::JavaScript => escape_js(text, quotes),
            Target::Html => escape_html(text),
            Target::Url => escape_url(text),
            Target::Shell => escape_shell(text),
            Target::Sql => escape_sql(text, quotes),
            Target::Regex => escape_regex(text),
        }),
        Mode::Unescape => match target {
            Target::Json => unescape_json(text),
            Target::JavaScript => unescape_js(text),
            Target::Html => unescape_html(text),
            Target::Url => unescape_url(text),
            Target::Shell | Target::Sql | Target::Regex => Err(format!(
                "unescape is not supported for target '{}' (it is a one-way escaping)",
                match target {
                    Target::Shell => "shell",
                    Target::Sql => "sql",
                    _ => "regex",
                }
            )),
        },
    }
}

/// String-keyed entry point used by the chat/web/CLI wrappers.
pub fn run(text: &str, target: &str, mode: &str, quotes: bool) -> Result<String, String> {
    let target = Target::parse(target)?;
    let mode = Mode::parse(mode)?;
    process(text, target, mode, quotes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_basic() {
        assert_eq!(
            run("He said \"hi\"\nbye", "json", "escape", false).unwrap(),
            "He said \\\"hi\\\"\\nbye"
        );
        assert_eq!(run("a\"b", "json", "escape", true).unwrap(), "\"a\\\"b\"");
    }

    #[test]
    fn json_roundtrip() {
        let raw = "tab\there\nand \"q\" \u{1F600}";
        let e = run(raw, "json", "escape", false).unwrap();
        assert_eq!(run(&e, "json", "unescape", false).unwrap(), raw);
    }

    #[test]
    fn js_escape_quotes_and_controls() {
        assert_eq!(
            run("it's a \"test\"\n", "javascript", "escape", false).unwrap(),
            "it\\'s a \\\"test\\\"\\n"
        );
        // wrap in quotes
        assert_eq!(
            run("a`b", "javascript", "escape", true).unwrap(),
            "\"a\\`b\""
        );
        // U+2028 must be escaped
        assert_eq!(
            run("\u{2028}", "javascript", "escape", false).unwrap(),
            "\\u2028"
        );
    }

    #[test]
    fn js_roundtrip() {
        let raw = "line1\nline2\t\"quoted\" 'single' \\back";
        let e = run(raw, "js", "escape", false).unwrap();
        assert_eq!(run(&e, "js", "unescape", false).unwrap(), raw);
        // \uXXXX and \xNN forms decode
        assert_eq!(run("\\u0041\\x42", "js", "unescape", false).unwrap(), "AB");
    }

    #[test]
    fn html_escape_and_unescape() {
        assert_eq!(
            run("<a href=\"x\">&'", "html", "escape", false).unwrap(),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(
            run("&lt;b&gt; &amp; &#39;q&#39; &#x41;", "html", "unescape", false).unwrap(),
            "<b> & 'q' A"
        );
    }

    #[test]
    fn html_unescape_unknown_entity_errors() {
        assert!(run("&bogus;", "html", "unescape", false).is_err());
    }

    #[test]
    fn url_escape_and_unescape() {
        assert_eq!(
            run("a b/c?=&\u{00E9}", "url", "escape", false).unwrap(),
            "a%20b%2Fc%3F%3D%26%C3%A9"
        );
        assert_eq!(
            run("S%C3%A3o+Paulo", "url", "unescape", false).unwrap(),
            "São Paulo"
        );
    }

    #[test]
    fn url_unescape_truncated_errors() {
        assert!(run("%2", "url", "unescape", false).is_err());
    }

    #[test]
    fn shell_single_quote_wrap() {
        assert_eq!(run("a b", "shell", "escape", false).unwrap(), "'a b'");
        assert_eq!(run("it's", "shell", "escape", false).unwrap(), "'it'\\''s'");
    }

    #[test]
    fn sql_doubles_quotes() {
        assert_eq!(run("O'Brien", "sql", "escape", false).unwrap(), "O''Brien");
        assert_eq!(run("O'Brien", "sql", "escape", true).unwrap(), "'O''Brien'");
    }

    #[test]
    fn regex_escapes_metachars() {
        assert_eq!(
            run("a.b*c+(d)", "regex", "escape", false).unwrap(),
            "a\\.b\\*c\\+\\(d\\)"
        );
    }

    #[test]
    fn one_way_targets_reject_unescape() {
        assert!(run("x", "shell", "unescape", false).is_err());
        assert!(run("x", "sql", "unescape", false).is_err());
        assert!(run("x", "regex", "unescape", false).is_err());
    }

    #[test]
    fn unknown_target_and_mode_error() {
        assert!(run("x", "yaml", "escape", false).is_err());
        assert!(run("x", "json", "frobnicate", false).is_err());
    }

    #[test]
    fn reversible_flags() {
        assert!(Target::Json.reversible());
        assert!(!Target::Shell.reversible());
    }
}
