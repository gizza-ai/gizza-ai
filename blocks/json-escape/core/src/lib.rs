//! gizza-ai/json-escape core — escape or unescape a string for safe use inside
//! JSON. Uses serde_json so the escaping exactly matches the JSON spec
//! (quotes, backslashes, control chars, \uXXXX). Pure-Rust.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Escape,
    Unescape,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "escape" | "" => Ok(Mode::Escape),
            "unescape" | "decode" => Ok(Mode::Unescape),
            other => Err(format!("unknown mode '{other}' (use escape or unescape)")),
        }
    }
}

/// Escape `text` into a JSON string body. When `quotes` is true the surrounding
/// double quotes are included.
pub fn escape(text: &str, quotes: bool) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    if quotes {
        quoted
    } else {
        // strip the leading and trailing '"'
        quoted[1..quoted.len() - 1].to_string()
    }
}

/// Unescape a JSON-escaped string back to its raw form. Accepts input with or
/// without the surrounding double quotes.
pub fn unescape(text: &str) -> Result<String, String> {
    let t = text.trim();
    let wrapped = if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t.to_string()
    } else {
        format!("\"{text}\"")
    };
    serde_json::from_str::<String>(&wrapped).map_err(|e| format!("invalid JSON string escaping: {e}"))
}

/// Dispatch by mode. In escape mode, `quotes` adds the surrounding quotes.
pub fn process(text: &str, mode: Mode, quotes: bool) -> Result<String, String> {
    match mode {
        Mode::Escape => Ok(escape(text, quotes)),
        Mode::Unescape => unescape(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_quotes_and_newlines() {
        let raw = "He said \"hi\"\nLine2\tTabbed";
        let e = escape(raw, false);
        assert_eq!(e, "He said \\\"hi\\\"\\nLine2\\tTabbed");
    }

    #[test]
    fn escape_with_quotes_wraps() {
        assert_eq!(escape("a\"b", true), "\"a\\\"b\"");
    }

    #[test]
    fn escapes_control_chars() {
        let e = escape("\u{0001}", false);
        assert_eq!(e, "\\u0001");
    }

    #[test]
    fn unescape_roundtrip() {
        let raw = "tab\there\nand \"quotes\" \u{1F600}";
        let e = escape(raw, false);
        assert_eq!(unescape(&e).unwrap(), raw);
    }

    #[test]
    fn unescape_accepts_quoted_or_bare() {
        assert_eq!(unescape("\"a\\nb\"").unwrap(), "a\nb");
        assert_eq!(unescape("a\\tb").unwrap(), "a\tb");
    }

    #[test]
    fn unescape_invalid() {
        assert!(unescape("a\\q").is_err()); // \q is not a valid escape
    }

    #[test]
    fn process_dispatch() {
        assert_eq!(process("a\nb", Mode::Escape, false).unwrap(), "a\\nb");
        assert_eq!(process("a\\nb", Mode::Unescape, false).unwrap(), "a\nb");
        assert!(Mode::parse("nope").is_err());
    }
}
