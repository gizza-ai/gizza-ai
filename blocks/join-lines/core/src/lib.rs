//! gizza-ai/join-lines core — join multiple lines of text into a single line.
//! Pure-Rust, no deps beyond serde. Shared by the chat skill block and the page.
//!
//! Takes a block of text and concatenates its lines using a chosen separator.
//! Optionally trims whitespace from each line, drops blank lines, and wraps
//! each line with a prefix and/or suffix before joining (handy for building
//! comma-separated lists, SQL `IN (...)` clauses, JSON-ish arrays, etc.).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Number of input lines (before any blank removal).
    pub total: usize,
    /// Number of lines actually joined (after optional blank removal).
    pub joined: usize,
    /// The single joined line.
    pub result: String,
}

/// Decode common backslash escapes in a user-supplied separator so the page /
/// chat can pass `\t`, `\n`, `\r` and a literal `\\` as a one-character field.
/// Any other `\x` is left as-is (the backslash is kept).
fn unescape(sep: &str) -> String {
    let mut out = String::with_capacity(sep.len());
    let mut chars = sep.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some('t') => {
                    out.push('\t');
                    chars.next();
                }
                Some('n') => {
                    out.push('\n');
                    chars.next();
                }
                Some('r') => {
                    out.push('\r');
                    chars.next();
                }
                Some('\\') => {
                    out.push('\\');
                    chars.next();
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Join the lines of `text` with `separator`.
///
/// * `separator` — placed between lines. Backslash escapes `\t \n \r \\` are
///   decoded so a single-line field can express tabs/newlines.
/// * `trim` — strip leading/trailing whitespace from each line first.
/// * `remove_blank` — drop blank/whitespace-only lines (evaluated after trim).
/// * `prefix` / `suffix` — wrap each surviving line before joining.
pub fn join(
    text: &str,
    separator: &str,
    trim: bool,
    remove_blank: bool,
    prefix: &str,
    suffix: &str,
) -> Result<Output, String> {
    let sep = unescape(separator);

    let raw: Vec<&str> = text.lines().collect();
    let total = raw.len();

    let mut pieces: Vec<String> = Vec::with_capacity(raw.len());
    for line in raw {
        let l = if trim { line.trim() } else { line };
        if remove_blank && l.trim().is_empty() {
            continue;
        }
        pieces.push(format!("{prefix}{l}{suffix}"));
    }

    let joined = pieces.len();
    Ok(Output {
        total,
        joined,
        result: pieces.join(&sep),
    })
}

/// Human-readable rendering used by the page — just the joined line.
pub fn render(
    text: &str,
    separator: &str,
    trim: bool,
    remove_blank: bool,
    prefix: &str,
    suffix: &str,
) -> Result<String, String> {
    Ok(join(text, separator, trim, remove_blank, prefix, suffix)?.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(text: &str, sep: &str, trim: bool, rb: bool, pre: &str, suf: &str) -> String {
        join(text, sep, trim, rb, pre, suf).unwrap().result
    }

    #[test]
    fn default_comma_space() {
        assert_eq!(r("a\nb\nc", ", ", false, false, "", ""), "a, b, c");
    }

    #[test]
    fn space_separator() {
        assert_eq!(r("one\ntwo\nthree", " ", false, false, "", ""), "one two three");
    }

    #[test]
    fn empty_separator_concatenates() {
        assert_eq!(r("ab\ncd\nef", "", false, false, "", ""), "abcdef");
    }

    #[test]
    fn tab_escape_separator() {
        assert_eq!(r("a\nb", "\\t", false, false, "", ""), "a\tb");
    }

    #[test]
    fn newline_escape_separator() {
        assert_eq!(r("a\nb", "\\n", false, false, "", ""), "a\nb");
    }

    #[test]
    fn literal_backslash() {
        assert_eq!(r("a\nb", "\\\\", false, false, "", ""), "a\\b");
    }

    #[test]
    fn unknown_escape_kept() {
        assert_eq!(r("a\nb", "\\x", false, false, "", ""), "a\\xb");
    }

    #[test]
    fn trim_each_line() {
        assert_eq!(r("  a  \n b ", ", ", true, false, "", ""), "a, b");
    }

    #[test]
    fn keeps_blanks_without_remove() {
        assert_eq!(r("a\n\nb", "|", false, false, "", ""), "a||b");
    }

    #[test]
    fn remove_blank_drops_empty() {
        assert_eq!(r("a\n\n  \nb", "|", false, true, "", ""), "a|b");
    }

    #[test]
    fn prefix_suffix_quotes() {
        assert_eq!(r("a\nb", ", ", false, false, "'", "'"), "'a', 'b'");
    }

    #[test]
    fn sql_in_clause() {
        let out = join("1\n2\n3", ", ", true, true, "(", ")").unwrap();
        assert_eq!(out.result, "(1), (2), (3)");
    }

    #[test]
    fn counts_reported() {
        let out = join("a\n\nb\nc", ", ", false, true, "", "").unwrap();
        assert_eq!(out.total, 4);
        assert_eq!(out.joined, 3);
        assert_eq!(out.result, "a, b, c");
    }

    #[test]
    fn single_line_unchanged() {
        assert_eq!(r("solo", ", ", false, false, "", ""), "solo");
    }

    #[test]
    fn empty_input() {
        let out = join("", ", ", false, false, "", "").unwrap();
        assert_eq!(out.total, 0);
        assert_eq!(out.joined, 0);
        assert_eq!(out.result, "");
    }
}
