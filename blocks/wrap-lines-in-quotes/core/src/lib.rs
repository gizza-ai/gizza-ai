//! gizza-ai/wrap-lines-in-quotes core — wrap each line of text in chosen quotes
//! or brackets, with an optional trailing separator. The classic "turn a column
//! of values into a SQL `IN (…)` list / JSON array / CSV row" helper.
//! Pure-Rust, no I/O.

use serde::Serialize;

/// A preset pair of opening/closing delimiters, or a fully custom pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapStyle {
    /// `"line"`
    Double,
    /// `'line'`
    Single,
    /// `` `line` ``
    Backtick,
    /// `(line)`
    Paren,
    /// `[line]`
    Square,
    /// `{line}`
    Curly,
    /// `<line>`
    Angle,
    /// `«line»`
    Guillemet,
    /// Use the caller-supplied `open`/`close` strings.
    Custom,
}

impl WrapStyle {
    pub fn parse(s: &str) -> Result<WrapStyle, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "double" | "\"" | "" => Ok(WrapStyle::Double),
            "single" | "'" => Ok(WrapStyle::Single),
            "backtick" | "backticks" | "`" => Ok(WrapStyle::Backtick),
            "paren" | "parens" | "parentheses" | "()" => Ok(WrapStyle::Paren),
            "square" | "bracket" | "brackets" | "[]" => Ok(WrapStyle::Square),
            "curly" | "brace" | "braces" | "{}" => Ok(WrapStyle::Curly),
            "angle" | "<>" => Ok(WrapStyle::Angle),
            "guillemet" | "guillemets" | "«»" => Ok(WrapStyle::Guillemet),
            "custom" => Ok(WrapStyle::Custom),
            other => Err(format!(
                "unknown wrap '{other}' (use double, single, backtick, paren, square, curly, angle, guillemet, or custom)"
            )),
        }
    }

    /// Resolve to the concrete (open, close) delimiter strings. For `Custom`,
    /// `open` is required; an empty `close` mirrors `open` (so a single char
    /// wraps both sides). Preset styles ignore `open`/`close`.
    pub fn delims(&self, open: &str, close: &str) -> Result<(String, String), String> {
        let pair = |o: &str, c: &str| (o.to_string(), c.to_string());
        Ok(match self {
            WrapStyle::Double => pair("\"", "\""),
            WrapStyle::Single => pair("'", "'"),
            WrapStyle::Backtick => pair("`", "`"),
            WrapStyle::Paren => pair("(", ")"),
            WrapStyle::Square => pair("[", "]"),
            WrapStyle::Curly => pair("{", "}"),
            WrapStyle::Angle => pair("<", ">"),
            WrapStyle::Guillemet => pair("«", "»"),
            WrapStyle::Custom => {
                if open.is_empty() {
                    return Err("custom wrap needs an 'open' delimiter".into());
                }
                let close = if close.is_empty() { open } else { close };
                pair(open, close)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total number of lines in the input.
    pub total: usize,
    /// How many lines were actually wrapped.
    pub wrapped: usize,
    /// The wrapped text.
    pub result: String,
}

/// Backslash-escape the delimiter characters (and `\` itself) inside a line so
/// the wrapped result is a valid quoted string literal.
fn escape_line(line: &str, open: &str, close: &str) -> String {
    let mut s = line.replace('\\', "\\\\");
    if !close.is_empty() {
        s = s.replace(close, &format!("\\{close}"));
    }
    if !open.is_empty() && open != close {
        s = s.replace(open, &format!("\\{open}"));
    }
    s
}

/// Wrap each line of `text` in `open`/`close`.
///
/// * `separator` — appended after each wrapped line (e.g. `,`). Empty = none.
/// * `last_line_separator` — when false, the last wrapped line gets no trailing
///   separator (so the output is a valid `IN (…)` / JSON-array body).
/// * `skip_empty` — when true, blank (whitespace-only) lines pass through
///   unchanged: no wrap, no separator.
/// * `trim` — strip surrounding whitespace from each line before wrapping.
/// * `escape` — backslash-escape the delimiter chars inside each line.
#[allow(clippy::too_many_arguments)]
pub fn wrap_lines(
    text: &str,
    open: &str,
    close: &str,
    separator: &str,
    last_line_separator: bool,
    skip_empty: bool,
    trim: bool,
    escape: bool,
) -> Result<Output, String> {
    let lines: Vec<&str> = text.lines().collect();

    // Which lines get wrapped (everything, unless skip_empty drops blanks).
    let flags: Vec<bool> = lines
        .iter()
        .map(|l| !(skip_empty && l.trim().is_empty()))
        .collect();
    let last_wrapped = flags.iter().rposition(|&w| w);

    let mut out = String::new();
    let mut wrapped = 0usize;
    for (i, (line, &do_wrap)) in lines.iter().zip(flags.iter()).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !do_wrap {
            // Blank line preserved as-is.
            out.push_str(line);
            continue;
        }
        wrapped += 1;
        let content = if trim { line.trim() } else { line };
        let content = if escape {
            escape_line(content, open, close)
        } else {
            content.to_string()
        };
        out.push_str(open);
        out.push_str(&content);
        out.push_str(close);
        let is_last = Some(i) == last_wrapped;
        if !separator.is_empty() && (!is_last || last_line_separator) {
            out.push_str(separator);
        }
    }

    Ok(Output {
        total: lines.len(),
        wrapped,
        result: out,
    })
}

/// Human-readable rendering (used by the page): resolves the style, then wraps.
#[allow(clippy::too_many_arguments)]
pub fn render(
    text: &str,
    wrap: &str,
    open: &str,
    close: &str,
    separator: &str,
    last_line_separator: bool,
    skip_empty: bool,
    trim: bool,
    escape: bool,
) -> Result<String, String> {
    let style = WrapStyle::parse(wrap)?;
    let (o, c) = style.delims(open, close)?;
    Ok(wrap_lines(
        text,
        &o,
        &c,
        separator,
        last_line_separator,
        skip_empty,
        trim,
        escape,
    )?
    .result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_double_quotes() {
        let o = wrap_lines("Apple\nBanana\nCherry", "\"", "\"", "", false, true, false, false)
            .unwrap();
        assert_eq!(o.result, "\"Apple\"\n\"Banana\"\n\"Cherry\"");
        assert_eq!(o.total, 3);
        assert_eq!(o.wrapped, 3);
    }

    #[test]
    fn separator_no_trailing_on_last() {
        // The SQL IN-list / JSON-array shape: comma after every line but the last.
        let o = wrap_lines("a\nb\nc", "\"", "\"", ",", false, true, false, false).unwrap();
        assert_eq!(o.result, "\"a\",\n\"b\",\n\"c\"");
    }

    #[test]
    fn separator_trailing_on_last_when_enabled() {
        let o = wrap_lines("a\nb", "\"", "\"", ",", true, true, false, false).unwrap();
        assert_eq!(o.result, "\"a\",\n\"b\",");
    }

    #[test]
    fn skip_empty_preserves_blanks_and_skips_them_for_separator() {
        // Blank line in the middle is preserved; the last *wrapped* line (b) is
        // the one that drops the trailing comma.
        let o = wrap_lines("a\n\nb", "\"", "\"", ",", false, true, false, false).unwrap();
        assert_eq!(o.result, "\"a\",\n\n\"b\"");
        assert_eq!(o.total, 3);
        assert_eq!(o.wrapped, 2);
    }

    #[test]
    fn wrap_all_lines_when_skip_empty_off() {
        let o = wrap_lines("a\n\nb", "[", "]", "", false, false, false, false).unwrap();
        assert_eq!(o.result, "[a]\n[]\n[b]");
        assert_eq!(o.wrapped, 3);
    }

    #[test]
    fn trim_strips_whitespace_before_wrapping() {
        let o = wrap_lines("  a  \n\tb", "'", "'", "", false, true, true, false).unwrap();
        assert_eq!(o.result, "'a'\n'b'");
    }

    #[test]
    fn escape_inner_quotes_and_backslash() {
        // 5" pipe -> "5\" pipe" ; a\b -> "a\\b"
        let o = wrap_lines("5\" pipe\na\\b", "\"", "\"", "", false, true, false, true).unwrap();
        assert_eq!(o.result, "\"5\\\" pipe\"\n\"a\\\\b\"");
    }

    #[test]
    fn brackets_and_custom_via_render() {
        assert_eq!(
            render("a\nb", "paren", "", "", "", false, true, false, false).unwrap(),
            "(a)\n(b)"
        );
        // Custom with a single char mirrors both sides.
        assert_eq!(
            render("a", "custom", "|", "", "", false, true, false, false).unwrap(),
            "|a|"
        );
        // Custom with distinct open/close.
        assert_eq!(
            render("x", "custom", "<<", ">>", "", false, true, false, false).unwrap(),
            "<<x>>"
        );
    }

    #[test]
    fn empty_input() {
        let o = wrap_lines("", "\"", "\"", ",", false, true, false, false).unwrap();
        assert_eq!(o.result, "");
        assert_eq!(o.total, 0);
        assert_eq!(o.wrapped, 0);
    }

    #[test]
    fn errors() {
        assert!(WrapStyle::parse("nope").is_err());
        // Custom without an open delimiter is an error.
        assert!(render("a", "custom", "", "", "", false, true, false, false).is_err());
    }
}
