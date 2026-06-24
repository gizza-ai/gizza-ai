//! split-text core — split text on a chosen delimiter into one item per line.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

/// How to interpret the `delimiter` string.
#[derive(Clone, Copy)]
enum Mode {
    /// `delimiter` is a literal substring to split on (default).
    Literal,
    /// Split on runs of any Unicode whitespace; `delimiter` is ignored.
    Whitespace,
    /// Split into individual characters (Unicode scalar values); `delimiter` is ignored.
    Chars,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "literal" => Ok(Mode::Literal),
            "whitespace" => Ok(Mode::Whitespace),
            "chars" => Ok(Mode::Chars),
            other => Err(format!(
                "invalid mode {other:?}: expected \"literal\", \"whitespace\", or \"chars\""
            )),
        }
    }
}

/// Split `text` into items and return them one per line.
///
/// - `delimiter`: the literal substring to split on (used only when
///   `mode == "literal"`). Common escapes are recognised: `\n`, `\t`, `\r`, and
///   `\\` (so `","`, `"\t"`, `", "` all work). A `\` followed by anything else is
///   kept verbatim (e.g. `\d` stays `\d`).
/// - `mode` (`"literal"` | `"whitespace"` | `"chars"`, blank → `"literal"`):
///     - `literal`: split on every occurrence of `delimiter`.
///     - `whitespace`: split on runs of whitespace (the delimiter is ignored);
///       leading/trailing whitespace produces no empty items.
///     - `chars`: emit each character on its own line (the delimiter is ignored).
/// - `trim`: trim leading/trailing whitespace from every item.
/// - `remove_empty`: drop items that are empty (after trimming, if `trim` is on).
///
/// Returns the items joined by `'\n'`. Returns `Err` on an invalid `mode`, or on
/// an empty `delimiter` in `literal` mode (splitting on "" is meaningless).
pub fn split(
    text: &str,
    delimiter: &str,
    mode: &str,
    trim: bool,
    remove_empty: bool,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;

    let mut items: Vec<String> = match mode {
        Mode::Literal => {
            let delim = unescape(delimiter);
            if delim.is_empty() {
                return Err(
                    "delimiter is empty: provide a delimiter, or use mode \"whitespace\"/\"chars\""
                        .to_string(),
                );
            }
            text.split(delim.as_str()).map(str::to_string).collect()
        }
        Mode::Whitespace => text.split_whitespace().map(str::to_string).collect(),
        Mode::Chars => text.chars().map(|c| c.to_string()).collect(),
    };

    if trim {
        for item in &mut items {
            *item = item.trim().to_string();
        }
    }
    if remove_empty {
        items.retain(|s| !s.is_empty());
    }

    Ok(items.join("\n"))
}

/// Translate the backslash escapes a user is likely to type in a single-line
/// field (`\n`, `\t`, `\r`, `\\`) into the real bytes. Any other `\x` is left
/// as the two literal characters so unknown escapes are never silently dropped.
fn unescape(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_comma() {
        assert_eq!(
            split("a,b,c", ",", "literal", false, false).unwrap(),
            "a\nb\nc"
        );
    }

    #[test]
    fn defaults_to_literal_when_mode_blank() {
        assert_eq!(split("a;b", ";", "", false, false).unwrap(), "a\nb");
    }

    #[test]
    fn multi_char_delimiter() {
        assert_eq!(
            split("a, b, c", ", ", "literal", false, false).unwrap(),
            "a\nb\nc"
        );
    }

    #[test]
    fn recognises_tab_escape() {
        assert_eq!(
            split("a\tb\tc", "\\t", "literal", false, false).unwrap(),
            "a\nb\nc"
        );
    }

    #[test]
    fn unknown_escape_kept_verbatim() {
        // "\d" is not a known escape, so it splits on the literal backslash-d.
        assert_eq!(
            split("a\\db\\dc", "\\d", "literal", false, false).unwrap(),
            "a\nb\nc"
        );
    }

    #[test]
    fn whitespace_mode_collapses_runs() {
        assert_eq!(
            split("  a   b\tc\n d  ", "", "whitespace", false, false).unwrap(),
            "a\nb\nc\nd"
        );
    }

    #[test]
    fn chars_mode_unicode() {
        assert_eq!(split("a✓b", "", "chars", false, false).unwrap(), "a\n✓\nb");
    }

    #[test]
    fn trim_each_item() {
        assert_eq!(
            split(" a , b ,c ", ",", "literal", true, false).unwrap(),
            "a\nb\nc"
        );
    }

    #[test]
    fn remove_empty_drops_blanks() {
        // "a,,b," -> a, "", b, "" -> a, b
        assert_eq!(
            split("a,,b,", ",", "literal", false, true).unwrap(),
            "a\nb"
        );
    }

    #[test]
    fn trim_then_remove_empty() {
        // A whitespace-only item becomes empty after trim, then is dropped.
        assert_eq!(
            split("a, ,b", ",", "literal", true, true).unwrap(),
            "a\nb"
        );
    }

    #[test]
    fn empty_literal_delimiter_errors() {
        let err = split("abc", "", "literal", false, false).unwrap_err();
        assert!(err.contains("delimiter is empty"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = split("abc", ",", "regex", false, false).unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn no_delimiter_match_yields_single_item() {
        assert_eq!(
            split("hello", ",", "literal", false, false).unwrap(),
            "hello"
        );
    }
}
